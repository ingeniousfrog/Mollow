use std::ffi::{CStr, CString};
use std::fs;
use std::io;
use std::mem::MaybeUninit;

use mollow_core::{
    Capability, CpuInfo, DataSource, MemoryInfo, RuntimeInfo, StorageVolume, SwapInfo, SystemInfo,
};

use crate::linux_parse::{parse_cpuinfo, parse_meminfo, parse_mountinfo, parse_os_release};
use crate::{PlatformProbe, ProbeArea, ProbeError, detect_runtimes};

pub struct NativeProbe;

impl PlatformProbe for NativeProbe {
    fn system(&self) -> Result<SystemInfo, ProbeError> {
        let release = fs::read_to_string("/etc/os-release")
            .map_err(|error| ProbeError::new("system", error.to_string()))?;
        let (os_name, os_version) = parse_os_release(&release);
        let kernel = uname().map_err(|error| ProbeError::new("system", error.to_string()))?;

        Ok(SystemInfo {
            os_name,
            os_version,
            kernel_version: Some(kernel.release),
            architecture: std::env::consts::ARCH.to_owned(),
            hostname: Some(kernel.hostname),
        })
    }

    fn cpu(&self) -> Result<CpuInfo, ProbeError> {
        let input = fs::read_to_string("/proc/cpuinfo")
            .map_err(|error| ProbeError::new("cpu", error.to_string()))?;
        let parsed = parse_cpuinfo(&input);
        let logical_cores = std::thread::available_parallelism()
            .map_err(|error| ProbeError::new("cpu", error.to_string()))?
            .get();

        Ok(CpuInfo {
            model: parsed.model,
            physical_cores: parsed.physical_cores,
            logical_cores: u32::try_from(logical_cores)
                .map_err(|error| ProbeError::new("cpu", error.to_string()))?,
            features: parsed.features,
        })
    }

    fn memory(&self) -> Result<MemoryInfo, ProbeError> {
        let input = fs::read_to_string("/proc/meminfo")
            .map_err(|error| ProbeError::new("memory", error.to_string()))?;
        let parsed = parse_meminfo(&input).map_err(|error| ProbeError::new("memory", error))?;

        Ok(MemoryInfo {
            total_bytes: parsed.total,
            available_bytes: parsed.available,
            swap: Capability::available(
                SwapInfo {
                    total_bytes: parsed.swap_total,
                    used_bytes: parsed.swap_used,
                },
                self.source(ProbeArea::Memory),
            ),
        })
    }

    fn storage(&self) -> Result<Vec<StorageVolume>, ProbeError> {
        let input = fs::read_to_string("/proc/self/mountinfo")
            .map_err(|error| ProbeError::new("storage", error.to_string()))?;
        let mounts = parse_mountinfo(&input).map_err(|error| ProbeError::new("storage", error))?;

        mounts
            .into_iter()
            .map(|mount| {
                let capacity = file_system_capacity(&mount.mount_point)
                    .map_err(|error| ProbeError::new("storage", error.to_string()))?;
                Ok(StorageVolume {
                    name: Some(mount.source),
                    mount_point: mount.mount_point,
                    file_system: Some(mount.file_system),
                    total_bytes: capacity.0,
                    available_bytes: capacity.1,
                    read_only: mount.read_only,
                })
            })
            .collect()
    }

    fn runtimes(&self) -> Result<Vec<RuntimeInfo>, ProbeError> {
        detect_runtimes()
    }

    fn source(&self, area: ProbeArea) -> DataSource {
        let (provider, detail) = match area {
            ProbeArea::System => ("linux-system", "/etc/os-release and uname"),
            ProbeArea::Cpu => ("linux-cpu", "/proc/cpuinfo"),
            ProbeArea::Memory => ("linux-memory", "/proc/meminfo"),
            ProbeArea::Storage => ("linux-storage", "/proc/self/mountinfo and statvfs"),
            ProbeArea::Runtimes => ("runtime-commands", "fixed version commands without a shell"),
        };

        DataSource {
            provider: provider.to_owned(),
            detail: Some(detail.to_owned()),
        }
    }
}

struct Uname {
    hostname: String,
    release: String,
}

fn uname() -> io::Result<Uname> {
    let mut value = MaybeUninit::<libc::utsname>::uninit();
    // SAFETY: `uname` initializes the complete `utsname` value on success.
    let status = unsafe { libc::uname(value.as_mut_ptr()) };
    if status != 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: A successful `uname` call initialized `value`.
    let value = unsafe { value.assume_init() };

    Ok(Uname {
        hostname: c_string(&value.nodename),
        release: c_string(&value.release),
    })
}

fn file_system_capacity(mount_point: &str) -> io::Result<(u64, u64)> {
    let mount_point = CString::new(mount_point)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    let mut statistics = MaybeUninit::<libc::statvfs>::uninit();
    // SAFETY: `mount_point` is NUL-terminated and `statvfs` initializes `statistics` on success.
    let status = unsafe { libc::statvfs(mount_point.as_ptr(), statistics.as_mut_ptr()) };
    if status != 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: A successful `statvfs` call initialized `statistics`.
    let statistics = unsafe { statistics.assume_init() };
    let block_size = statistics.f_frsize;
    let total_bytes = statistics
        .f_blocks
        .checked_mul(block_size)
        .ok_or_else(|| io::Error::other("storage capacity overflowed"))?;
    let available_bytes = statistics
        .f_bavail
        .checked_mul(block_size)
        .ok_or_else(|| io::Error::other("available storage capacity overflowed"))?;

    Ok((total_bytes, available_bytes))
}

fn c_string<const N: usize>(value: &[libc::c_char; N]) -> String {
    // SAFETY: `utsname` fields returned by `uname` are NUL-terminated arrays.
    unsafe { CStr::from_ptr(value.as_ptr()) }
        .to_string_lossy()
        .into_owned()
}
