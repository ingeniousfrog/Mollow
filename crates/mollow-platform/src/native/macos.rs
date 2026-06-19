use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::io;
use std::mem::size_of;
use std::ptr;
use std::slice;

use mollow_core::{
    Capability, CpuInfo, DataSource, MemoryInfo, RuntimeInfo, StorageVolume, SwapInfo, SystemInfo,
};

use crate::{PlatformProbe, ProbeArea, ProbeError, detect_runtimes};

const HOST_VM_INFO64: c_int = 4;

type MachPort = u32;
type KernReturn = c_int;
type MachMessageTypeNumber = u32;

unsafe extern "C" {
    fn sysctlbyname(
        name: *const c_char,
        old_value: *mut c_void,
        old_length: *mut usize,
        new_value: *mut c_void,
        new_length: usize,
    ) -> c_int;
    fn mach_host_self() -> MachPort;
    fn host_page_size(host: MachPort, page_size: *mut u32) -> KernReturn;
    fn host_statistics64(
        host: MachPort,
        flavor: c_int,
        host_info: *mut c_int,
        host_info_count: *mut MachMessageTypeNumber,
    ) -> KernReturn;
}

#[derive(Default)]
#[repr(C)]
struct VmStatistics64 {
    free_count: u32,
    active_count: u32,
    inactive_count: u32,
    wire_count: u32,
    zero_fill_count: u64,
    reactivations: u64,
    pageins: u64,
    pageouts: u64,
    faults: u64,
    copy_on_write_faults: u64,
    lookups: u64,
    hits: u64,
    purges: u64,
    purgeable_count: u32,
    speculative_count: u32,
    decompressions: u64,
    compressions: u64,
    swapins: u64,
    swapouts: u64,
    compressor_page_count: u32,
    throttled_count: u32,
    external_page_count: u32,
    internal_page_count: u32,
    total_uncompressed_pages_in_compressor: u64,
}

#[derive(Default)]
#[repr(C)]
struct SwapUsage {
    total: u64,
    available: u64,
    used: u64,
    page_size: u32,
    encrypted: c_int,
}

pub struct NativeProbe;

impl PlatformProbe for NativeProbe {
    fn system(&self) -> Result<SystemInfo, ProbeError> {
        let read = |name| {
            sysctl_string(name).map_err(|error| ProbeError::new("system", error.to_string()))
        };

        Ok(SystemInfo {
            os_name: "macOS".to_owned(),
            os_version: Some(read("kern.osproductversion")?),
            kernel_version: Some(read("kern.osrelease")?),
            architecture: std::env::consts::ARCH.to_owned(),
            hostname: Some(read("kern.hostname")?),
        })
    }

    fn cpu(&self) -> Result<CpuInfo, ProbeError> {
        let logical_cores = sysctl_u32("hw.logicalcpu")
            .or_else(|_| {
                u32::try_from(
                    std::thread::available_parallelism()
                        .map_err(|error| io::Error::other(error.to_string()))?
                        .get(),
                )
                .map_err(|error| io::Error::other(error.to_string()))
            })
            .map_err(|error| ProbeError::new("cpu", error.to_string()))?;
        let model = sysctl_string("machdep.cpu.brand_string")
            .or_else(|_| sysctl_string("hw.model"))
            .map_err(|error| ProbeError::new("cpu", error.to_string()))?;
        let physical_cores = sysctl_u32("hw.physicalcpu")
            .map_err(|error| ProbeError::new("cpu", error.to_string()))?;
        let features = cpu_features();

        Ok(CpuInfo {
            model: Some(model),
            physical_cores: Some(physical_cores),
            logical_cores,
            features,
        })
    }

    fn memory(&self) -> Result<MemoryInfo, ProbeError> {
        let total_bytes = sysctl_u64("hw.memsize")
            .map_err(|error| ProbeError::new("memory", error.to_string()))?;
        let available_bytes = available_memory_bytes()
            .map_err(|error| ProbeError::new("memory", error.to_string()))?;
        let swap = match swap_usage() {
            Ok(swap) => Capability::available(
                SwapInfo {
                    total_bytes: swap.total,
                    used_bytes: swap.used,
                },
                self.source(ProbeArea::Memory),
            ),
            Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {
                Capability::permission_denied(error.to_string())
            }
            Err(error) => Capability::error(error.to_string()),
        };

        Ok(MemoryInfo {
            total_bytes,
            available_bytes: Some(available_bytes.min(total_bytes)),
            swap,
        })
    }

    fn storage(&self) -> Result<Vec<StorageVolume>, ProbeError> {
        mounted_volumes().map_err(|error| ProbeError::new("storage", error.to_string()))
    }

    fn runtimes(&self) -> Result<Vec<RuntimeInfo>, ProbeError> {
        detect_runtimes()
    }

    fn source(&self, area: ProbeArea) -> DataSource {
        let (provider, detail) = match area {
            ProbeArea::System | ProbeArea::Cpu => ("macos-sysctl", "sysctlbyname FFI"),
            ProbeArea::Memory => ("macos-memory", "sysctlbyname and Mach host statistics"),
            ProbeArea::Storage => ("macos-storage", "getmntinfo"),
            ProbeArea::Runtimes => ("runtime-commands", "fixed version commands without a shell"),
        };

        DataSource {
            provider: provider.to_owned(),
            detail: Some(detail.to_owned()),
        }
    }
}

fn cpu_features() -> Vec<String> {
    let mut features = Vec::new();

    #[cfg(target_arch = "aarch64")]
    {
        let candidates = [
            ("neon", std::arch::is_aarch64_feature_detected!("neon")),
            ("aes", std::arch::is_aarch64_feature_detected!("aes")),
            ("sha2", std::arch::is_aarch64_feature_detected!("sha2")),
            ("crc", std::arch::is_aarch64_feature_detected!("crc")),
            ("lse", std::arch::is_aarch64_feature_detected!("lse")),
        ];
        features.extend(
            candidates
                .into_iter()
                .filter(|(_, available)| *available)
                .map(|(name, _)| name.to_owned()),
        );
    }

    #[cfg(target_arch = "x86_64")]
    {
        let candidates = [
            ("sse4.2", std::arch::is_x86_feature_detected!("sse4.2")),
            ("avx", std::arch::is_x86_feature_detected!("avx")),
            ("avx2", std::arch::is_x86_feature_detected!("avx2")),
            ("aes", std::arch::is_x86_feature_detected!("aes")),
        ];
        features.extend(
            candidates
                .into_iter()
                .filter(|(_, available)| *available)
                .map(|(name, _)| name.to_owned()),
        );
    }

    features
}

fn available_memory_bytes() -> io::Result<u64> {
    let mut statistics = VmStatistics64::default();
    let mut count = u32::try_from(size_of::<VmStatistics64>() / size_of::<c_int>())
        .map_err(|error| io::Error::other(error.to_string()))?;
    // SAFETY: This returns the send right for the current task's host port.
    let host = unsafe { mach_host_self() };
    let mut page_size = 0_u32;
    // SAFETY: `page_size` is valid writable storage and `host` is from `mach_host_self`.
    let status = unsafe { host_page_size(host, &raw mut page_size) };
    if status != 0 {
        return Err(io::Error::other(format!(
            "host_page_size returned {status}"
        )));
    }

    // SAFETY: `statistics` is writable storage with the count required by HOST_VM_INFO64.
    let status = unsafe {
        host_statistics64(
            host,
            HOST_VM_INFO64,
            (&raw mut statistics).cast::<c_int>(),
            &raw mut count,
        )
    };
    if status != 0 {
        return Err(io::Error::other(format!(
            "host_statistics64 returned {status}"
        )));
    }

    let available_pages = u64::from(statistics.free_count)
        .saturating_add(u64::from(statistics.inactive_count))
        .saturating_add(u64::from(statistics.speculative_count))
        .saturating_add(u64::from(statistics.purgeable_count));

    available_pages
        .checked_mul(u64::from(page_size))
        .ok_or_else(|| io::Error::other("available memory size overflowed"))
}

fn swap_usage() -> io::Result<SwapUsage> {
    let mut usage = SwapUsage::default();
    read_sysctl_number("vm.swapusage", &raw mut usage)?;
    Ok(usage)
}

fn mounted_volumes() -> io::Result<Vec<StorageVolume>> {
    let mut mounts = ptr::null_mut::<libc::statfs>();
    // SAFETY: `getmntinfo` initializes `mounts` with an OS-owned array valid until the next call.
    let count = unsafe { libc::getmntinfo(&raw mut mounts, libc::MNT_NOWAIT) };
    if count <= 0 || mounts.is_null() {
        return Err(io::Error::last_os_error());
    }

    let count = usize::try_from(count).map_err(|error| io::Error::other(error.to_string()))?;
    // SAFETY: `getmntinfo` returned `count` contiguous `statfs` entries at `mounts`.
    let entries = unsafe { slice::from_raw_parts(mounts, count) };

    entries.iter().map(storage_volume).collect()
}

fn storage_volume(entry: &libc::statfs) -> io::Result<StorageVolume> {
    let block_size = u64::from(entry.f_bsize);
    let total_bytes = entry
        .f_blocks
        .checked_mul(block_size)
        .ok_or_else(|| io::Error::other("storage capacity overflowed"))?;
    let available_bytes = entry
        .f_bavail
        .checked_mul(block_size)
        .ok_or_else(|| io::Error::other("available storage capacity overflowed"))?;
    let read_only_flag = u32::try_from(libc::MNT_RDONLY)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;

    Ok(StorageVolume {
        name: c_array_string(&entry.f_mntfromname),
        mount_point: c_array_string(&entry.f_mntonname).unwrap_or_default(),
        file_system: c_array_string(&entry.f_fstypename),
        total_bytes,
        available_bytes,
        read_only: entry.f_flags & read_only_flag != 0,
    })
}

fn c_array_string<const N: usize>(value: &[c_char; N]) -> Option<String> {
    // SAFETY: Darwin mount strings are fixed-size NUL-terminated arrays.
    let value = unsafe { CStr::from_ptr(value.as_ptr()) };
    let value = value.to_string_lossy().trim().to_owned();
    (!value.is_empty()).then_some(value)
}

fn sysctl_string(name: &str) -> io::Result<String> {
    let name = c_name(name)?;
    let mut length = 0_usize;

    // SAFETY: `name` is a valid NUL-terminated C string and the first call only writes `length`.
    let status = unsafe {
        sysctlbyname(
            name.as_ptr(),
            ptr::null_mut(),
            &raw mut length,
            ptr::null_mut(),
            0,
        )
    };
    if status != 0 {
        return Err(io::Error::last_os_error());
    }

    let mut bytes = vec![0_u8; length];
    // SAFETY: `bytes` owns `length` writable bytes and all pointers remain valid for the call.
    let status = unsafe {
        sysctlbyname(
            name.as_ptr(),
            bytes.as_mut_ptr().cast::<c_void>(),
            &raw mut length,
            ptr::null_mut(),
            0,
        )
    };
    if status != 0 {
        return Err(io::Error::last_os_error());
    }

    bytes.truncate(length);
    if bytes.last() == Some(&0) {
        bytes.pop();
    }

    String::from_utf8(bytes).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

fn sysctl_u32(name: &str) -> io::Result<u32> {
    let mut value = 0_u32;
    read_sysctl_number(name, &raw mut value)?;
    Ok(value)
}

fn sysctl_u64(name: &str) -> io::Result<u64> {
    let mut value = 0_u64;
    read_sysctl_number(name, &raw mut value)?;
    Ok(value)
}

fn read_sysctl_number<T>(name: &str, value: *mut T) -> io::Result<()> {
    let name = c_name(name)?;
    let mut length = size_of::<T>();

    // SAFETY: `value` points to initialized writable storage of `size_of::<T>()` bytes.
    let status = unsafe {
        sysctlbyname(
            name.as_ptr(),
            value.cast::<c_void>(),
            &raw mut length,
            ptr::null_mut(),
            0,
        )
    };
    if status != 0 {
        return Err(io::Error::last_os_error());
    }
    if length != size_of::<T>() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unexpected sysctl value size: {length}"),
        ));
    }

    Ok(())
}

fn c_name(name: &str) -> io::Result<CString> {
    CString::new(name).map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))
}
