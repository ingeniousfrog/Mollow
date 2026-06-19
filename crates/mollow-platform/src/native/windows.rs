use std::ffi::c_void;
use std::io;
use std::mem::size_of;
use std::ptr;

use mollow_core::{
    Capability, CpuInfo, DataSource, MemoryInfo, PowerInfo, RuntimeInfo, StorageVolume, SwapInfo,
    SystemInfo,
};

use crate::{PlatformProbe, ProbeArea, ProbeError, detect_runtimes};

const ALL_PROCESSOR_GROUPS: u16 = 0xffff;
const ERROR_SUCCESS: i32 = 0;
const FILE_READ_ONLY_VOLUME: u32 = 0x0008_0000;
const HKEY_LOCAL_MACHINE: isize = -2_147_483_646;
const RRF_RT_REG_SZ: u32 = 0x0000_0002;

#[repr(C)]
struct MemoryStatusEx {
    length: u32,
    memory_load: u32,
    total_physical: u64,
    available_physical: u64,
    total_page_file: u64,
    available_page_file: u64,
    total_virtual: u64,
    available_virtual: u64,
    available_extended_virtual: u64,
}

#[repr(C)]
struct OsVersionInfoEx {
    size: u32,
    major: u32,
    minor: u32,
    build: u32,
    platform_id: u32,
    service_pack: [u16; 128],
    service_pack_major: u16,
    service_pack_minor: u16,
    suite_mask: u16,
    product_type: u8,
    reserved: u8,
}

#[repr(C)]
struct SystemPowerStatus {
    ac_line_status: u8,
    battery_flag: u8,
    battery_life_percent: u8,
    system_status_flag: u8,
    battery_life_time: u32,
    battery_full_life_time: u32,
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetActiveProcessorCount(group_number: u16) -> u32;
    fn GetComputerNameW(buffer: *mut u16, size: *mut u32) -> i32;
    fn GetDiskFreeSpaceExW(
        directory: *const u16,
        free_for_caller: *mut u64,
        total: *mut u64,
        total_free: *mut u64,
    ) -> i32;
    fn GetLogicalProcessorInformationEx(
        relationship_type: u32,
        buffer: *mut c_void,
        returned_length: *mut u32,
    ) -> i32;
    fn GetLogicalDriveStringsW(buffer_length: u32, buffer: *mut u16) -> u32;
    fn GetVolumeInformationW(
        root: *const u16,
        volume_name: *mut u16,
        volume_name_size: u32,
        serial_number: *mut u32,
        maximum_component_length: *mut u32,
        file_system_flags: *mut u32,
        file_system_name: *mut u16,
        file_system_name_size: u32,
    ) -> i32;
    fn IsProcessorFeaturePresent(feature: u32) -> i32;
    fn GlobalMemoryStatusEx(status: *mut MemoryStatusEx) -> i32;
    fn GetSystemPowerStatus(status: *mut SystemPowerStatus) -> i32;
}

#[link(name = "advapi32")]
unsafe extern "system" {
    fn RegGetValueW(
        key: isize,
        sub_key: *const u16,
        value: *const u16,
        flags: u32,
        value_type: *mut u32,
        data: *mut c_void,
        data_size: *mut u32,
    ) -> i32;
}

#[link(name = "ntdll")]
unsafe extern "system" {
    fn RtlGetVersion(version: *mut OsVersionInfoEx) -> i32;
}

pub struct NativeProbe;

impl PlatformProbe for NativeProbe {
    fn system(&self) -> Result<SystemInfo, ProbeError> {
        let version =
            windows_version().map_err(|error| ProbeError::new("system", error.to_string()))?;
        let hostname = hostname().map_err(|error| ProbeError::new("system", error.to_string()))?;

        Ok(SystemInfo {
            os_name: "Windows".to_owned(),
            os_version: Some(version),
            kernel_version: None,
            architecture: std::env::consts::ARCH.to_owned(),
            hostname: Some(hostname),
        })
    }

    fn cpu(&self) -> Result<CpuInfo, ProbeError> {
        // SAFETY: The constant requests the count across all processor groups.
        let logical_cores = unsafe { GetActiveProcessorCount(ALL_PROCESSOR_GROUPS) };
        if logical_cores == 0 {
            return Err(ProbeError::new(
                "cpu",
                io::Error::last_os_error().to_string(),
            ));
        }

        Ok(CpuInfo {
            model: Some(
                registry_string(
                    r"HARDWARE\DESCRIPTION\System\CentralProcessor\0",
                    "ProcessorNameString",
                )
                .map_err(|error| ProbeError::new("cpu", error.to_string()))?,
            ),
            physical_cores: Some(
                physical_core_count().map_err(|error| ProbeError::new("cpu", error.to_string()))?,
            ),
            logical_cores,
            features: processor_features(),
        })
    }

    fn memory(&self) -> Result<MemoryInfo, ProbeError> {
        let status =
            memory_status().map_err(|error| ProbeError::new("memory", error.to_string()))?;
        let swap_total = status.total_page_file.saturating_sub(status.total_physical);
        let swap_available = status
            .available_page_file
            .saturating_sub(status.available_physical);

        Ok(MemoryInfo {
            total_bytes: status.total_physical,
            available_bytes: Some(status.available_physical),
            swap: Capability::available(
                SwapInfo {
                    total_bytes: swap_total,
                    used_bytes: swap_total.saturating_sub(swap_available),
                },
                self.source(ProbeArea::Memory),
            ),
        })
    }

    fn storage(&self) -> Result<Vec<StorageVolume>, ProbeError> {
        logical_drives()
            .and_then(|drives| {
                drives
                    .into_iter()
                    .map(|drive| storage_volume(&drive))
                    .collect()
            })
            .map_err(|error| ProbeError::new("storage", error.to_string()))
    }

    fn runtimes(&self) -> Result<Vec<RuntimeInfo>, ProbeError> {
        detect_runtimes()
    }

    fn power(&self) -> Capability<PowerInfo> {
        windows_power().map_or_else(
            |error| Capability::error(error.to_string()),
            |power| Capability::available(power, self.source(ProbeArea::Power)),
        )
    }

    fn source(&self, area: ProbeArea) -> DataSource {
        let (provider, detail) = match area {
            ProbeArea::System => ("windows-system", "RtlGetVersion and GetComputerNameW"),
            ProbeArea::Cpu => (
                "windows-cpu",
                "processor groups, feature flags, and registry",
            ),
            ProbeArea::Memory => ("windows-memory", "GlobalMemoryStatusEx"),
            ProbeArea::Storage => ("windows-storage", "Win32 volume APIs"),
            ProbeArea::Runtimes => ("runtime-commands", "fixed version commands without a shell"),
            ProbeArea::Gpu => ("windows-gpu", "not implemented"),
            ProbeArea::Media => ("windows-media", "not implemented"),
            ProbeArea::Power => ("windows-power", "GetSystemPowerStatus"),
            ProbeArea::Thermal => ("windows-thermal", "not implemented"),
        };

        DataSource {
            provider: provider.to_owned(),
            detail: Some(detail.to_owned()),
        }
    }
}

fn windows_power() -> io::Result<PowerInfo> {
    let mut status = SystemPowerStatus {
        ac_line_status: 255,
        battery_flag: 255,
        battery_life_percent: 255,
        system_status_flag: 0,
        battery_life_time: u32::MAX,
        battery_full_life_time: u32::MAX,
    };
    // SAFETY: `status` points to writable storage matching SYSTEM_POWER_STATUS.
    if unsafe { GetSystemPowerStatus(&raw mut status) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(PowerInfo {
        source: match status.ac_line_status {
            0 => "battery",
            1 => "ac",
            _ => "unknown",
        }
        .to_owned(),
        battery_percent: (status.battery_life_percent <= 100)
            .then_some(status.battery_life_percent),
        charging: if status.battery_flag & 8 != 0 {
            Some(true)
        } else if status.battery_flag == 255 {
            None
        } else {
            Some(false)
        },
        low_power_mode: Some(status.system_status_flag != 0),
    })
}

fn windows_version() -> io::Result<String> {
    let size = u32::try_from(size_of::<OsVersionInfoEx>())
        .map_err(|error| io::Error::other(error.to_string()))?;
    let mut version = OsVersionInfoEx {
        size,
        major: 0,
        minor: 0,
        build: 0,
        platform_id: 0,
        service_pack: [0; 128],
        service_pack_major: 0,
        service_pack_minor: 0,
        suite_mask: 0,
        product_type: 0,
        reserved: 0,
    };
    // SAFETY: `version` is writable and its size field matches the structure.
    let status = unsafe { RtlGetVersion(&raw mut version) };
    if status != 0 {
        return Err(io::Error::other(format!("RtlGetVersion returned {status}")));
    }

    Ok(format!(
        "{}.{}.{}",
        version.major, version.minor, version.build
    ))
}

fn hostname() -> io::Result<String> {
    let mut buffer = vec![0_u16; 256];
    let mut length =
        u32::try_from(buffer.len()).map_err(|error| io::Error::other(error.to_string()))?;
    // SAFETY: `buffer` contains `length` writable UTF-16 code units.
    let status = unsafe { GetComputerNameW(buffer.as_mut_ptr(), &raw mut length) };
    if status == 0 {
        return Err(io::Error::last_os_error());
    }
    let length = usize::try_from(length).map_err(|error| io::Error::other(error.to_string()))?;

    String::from_utf16(&buffer[..length])
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

fn memory_status() -> io::Result<MemoryStatusEx> {
    let length = u32::try_from(size_of::<MemoryStatusEx>())
        .map_err(|error| io::Error::other(error.to_string()))?;
    let mut status = MemoryStatusEx {
        length,
        memory_load: 0,
        total_physical: 0,
        available_physical: 0,
        total_page_file: 0,
        available_page_file: 0,
        total_virtual: 0,
        available_virtual: 0,
        available_extended_virtual: 0,
    };
    // SAFETY: `status` is writable and its length field matches the structure.
    if unsafe { GlobalMemoryStatusEx(&raw mut status) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(status)
}

fn processor_features() -> Vec<String> {
    const FEATURES: &[(u32, &str)] = &[
        (3, "mmx"),
        (6, "sse"),
        (10, "sse2"),
        (13, "sse3"),
        (18, "avx"),
        (19, "neon"),
        (29, "armv8"),
        (30, "armv8-crypto"),
        (31, "armv8-crc32"),
    ];

    FEATURES
        .iter()
        .filter(|(feature, _)| {
            // SAFETY: The feature identifiers are documented Win32 constants.
            unsafe { IsProcessorFeaturePresent(*feature) != 0 }
        })
        .map(|(_, name)| (*name).to_owned())
        .collect()
}

fn physical_core_count() -> io::Result<u32> {
    const RELATION_PROCESSOR_CORE: u32 = 0;
    const HEADER_SIZE: usize = size_of::<u32>() * 2;

    let mut byte_length = 0_u32;
    // SAFETY: A null buffer query writes only the required byte length.
    unsafe {
        GetLogicalProcessorInformationEx(
            RELATION_PROCESSOR_CORE,
            ptr::null_mut(),
            &raw mut byte_length,
        );
    }
    if byte_length == 0 {
        return Err(io::Error::last_os_error());
    }

    let length =
        usize::try_from(byte_length).map_err(|error| io::Error::other(error.to_string()))?;
    let mut buffer = vec![0_u8; length];
    // SAFETY: `buffer` owns `byte_length` writable bytes.
    let status = unsafe {
        GetLogicalProcessorInformationEx(
            RELATION_PROCESSOR_CORE,
            buffer.as_mut_ptr().cast::<c_void>(),
            &raw mut byte_length,
        )
    };
    if status == 0 {
        return Err(io::Error::last_os_error());
    }

    let mut offset = 0_usize;
    let mut core_count = 0_u32;
    while offset < length {
        if length - offset < HEADER_SIZE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "processor information contains a truncated header",
            ));
        }
        // SAFETY: The bounds check above guarantees two readable u32 fields.
        let relationship =
            unsafe { ptr::read_unaligned(buffer.as_ptr().add(offset).cast::<u32>()) };
        // SAFETY: The bounds check above guarantees the size field is readable.
        let entry_size =
            unsafe { ptr::read_unaligned(buffer.as_ptr().add(offset + 4).cast::<u32>()) };
        let entry_size =
            usize::try_from(entry_size).map_err(|error| io::Error::other(error.to_string()))?;
        if entry_size < HEADER_SIZE || offset.saturating_add(entry_size) > length {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "processor information contains an invalid entry size",
            ));
        }
        if relationship == RELATION_PROCESSOR_CORE {
            core_count = core_count
                .checked_add(1)
                .ok_or_else(|| io::Error::other("physical core count overflowed"))?;
        }
        offset += entry_size;
    }

    if core_count == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Windows reported no physical processor cores",
        ));
    }
    Ok(core_count)
}

fn registry_string(sub_key: &str, value: &str) -> io::Result<String> {
    let sub_key = wide_null(sub_key);
    let value = wide_null(value);
    let mut byte_length = 0_u32;
    // SAFETY: The key handle and string pointers are valid; the first call only writes size.
    let status = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            sub_key.as_ptr(),
            value.as_ptr(),
            RRF_RT_REG_SZ,
            ptr::null_mut(),
            ptr::null_mut(),
            &raw mut byte_length,
        )
    };
    if status != ERROR_SUCCESS {
        return Err(io::Error::from_raw_os_error(status));
    }
    let units = usize::try_from(byte_length)
        .map_err(|error| io::Error::other(error.to_string()))?
        .div_ceil(size_of::<u16>());
    let mut buffer = vec![0_u16; units];
    // SAFETY: `buffer` owns `byte_length` writable bytes and pointers remain valid.
    let status = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            sub_key.as_ptr(),
            value.as_ptr(),
            RRF_RT_REG_SZ,
            ptr::null_mut(),
            buffer.as_mut_ptr().cast::<c_void>(),
            &raw mut byte_length,
        )
    };
    if status != ERROR_SUCCESS {
        return Err(io::Error::from_raw_os_error(status));
    }

    Ok(String::from_utf16_lossy(trim_wide_null(&buffer))
        .trim()
        .to_owned())
}

fn logical_drives() -> io::Result<Vec<String>> {
    // SAFETY: A zero-size query with a null buffer returns the required length.
    let required = unsafe { GetLogicalDriveStringsW(0, ptr::null_mut()) };
    if required == 0 {
        return Err(io::Error::last_os_error());
    }
    let length = usize::try_from(required).map_err(|error| io::Error::other(error.to_string()))?;
    let mut buffer = vec![0_u16; length];
    // SAFETY: `buffer` contains `required` writable UTF-16 code units.
    let written = unsafe { GetLogicalDriveStringsW(required, buffer.as_mut_ptr()) };
    if written == 0 || written > required {
        return Err(io::Error::last_os_error());
    }

    Ok(buffer
        .split(|unit| *unit == 0)
        .filter(|drive| !drive.is_empty())
        .map(String::from_utf16_lossy)
        .collect())
}

fn storage_volume(root: &str) -> io::Result<StorageVolume> {
    let root_wide = wide_null(root);
    let mut total_bytes = 0_u64;
    let mut available_bytes = 0_u64;
    // SAFETY: `root_wide` is NUL-terminated and output pointers are writable.
    let status = unsafe {
        GetDiskFreeSpaceExW(
            root_wide.as_ptr(),
            &raw mut available_bytes,
            &raw mut total_bytes,
            ptr::null_mut(),
        )
    };
    if status == 0 {
        return Err(io::Error::last_os_error());
    }

    let mut volume_name = vec![0_u16; 261];
    let mut file_system_name = vec![0_u16; 261];
    let mut flags = 0_u32;
    let volume_length =
        u32::try_from(volume_name.len()).map_err(|error| io::Error::other(error.to_string()))?;
    let file_system_length = u32::try_from(file_system_name.len())
        .map_err(|error| io::Error::other(error.to_string()))?;
    // SAFETY: All buffers are writable and lengths match their allocations.
    let status = unsafe {
        GetVolumeInformationW(
            root_wide.as_ptr(),
            volume_name.as_mut_ptr(),
            volume_length,
            ptr::null_mut(),
            ptr::null_mut(),
            &raw mut flags,
            file_system_name.as_mut_ptr(),
            file_system_length,
        )
    };
    if status == 0 {
        return Err(io::Error::last_os_error());
    }

    Ok(StorageVolume {
        name: nonempty_wide(&volume_name),
        mount_point: root.to_owned(),
        file_system: nonempty_wide(&file_system_name),
        total_bytes,
        available_bytes,
        read_only: flags & FILE_READ_ONLY_VOLUME != 0,
    })
}

fn wide_null(value: &str) -> Vec<u16> {
    value.encode_utf16().chain([0]).collect()
}

fn trim_wide_null(value: &[u16]) -> &[u16] {
    value.split(|unit| *unit == 0).next().unwrap_or_default()
}

fn nonempty_wide(value: &[u16]) -> Option<String> {
    let value = String::from_utf16_lossy(trim_wide_null(value))
        .trim()
        .to_owned();
    (!value.is_empty()).then_some(value)
}
