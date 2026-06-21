use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::io;
use std::mem::size_of;
use std::process::Command;
use std::ptr;
use std::slice;

use mollow_core::{
    Capability, CpuInfo, DataSource, GpuInfo, MediaInfo, MemoryInfo, MemoryModuleInfo, PowerInfo,
    RuntimeInfo, StorageVolume, SwapInfo, SystemInfo, ThermalInfo,
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

#[link(name = "VideoToolbox", kind = "framework")]
unsafe extern "C" {
    fn VTIsHardwareDecodeSupported(codec_type: u32) -> u8;
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
            modules: memory_modules_capability(self),
        })
    }

    fn storage(&self) -> Result<Vec<StorageVolume>, ProbeError> {
        mounted_volumes().map_err(|error| ProbeError::new("storage", error.to_string()))
    }

    fn runtimes(&self) -> Result<Vec<RuntimeInfo>, ProbeError> {
        detect_runtimes()
    }

    fn gpu(&self) -> Capability<Vec<GpuInfo>> {
        run_command(
            "/usr/sbin/system_profiler",
            &["-json", "SPDisplaysDataType"],
        )
        .and_then(|output| parse_gpu_json(&output))
        .map_or_else(
            |error| Capability::error(error.to_string()),
            |value| Capability::available(value, self.source(ProbeArea::Gpu)),
        )
    }

    fn media(&self) -> Capability<MediaInfo> {
        let codecs = [
            ("h264", fourcc(*b"avc1")),
            ("hevc", fourcc(*b"hvc1")),
            ("vp9", fourcc(*b"vp09")),
            ("av1", fourcc(*b"av01")),
        ];
        let hardware_decode_codecs = codecs
            .into_iter()
            .filter(|(_, codec)| {
                // SAFETY: Codec values are valid CoreMedia four-character codes.
                unsafe { VTIsHardwareDecodeSupported(*codec) != 0 }
            })
            .map(|(name, _)| name.to_owned())
            .collect();
        Capability::available(
            MediaInfo {
                backend: "VideoToolbox".to_owned(),
                hardware_decode_codecs,
                hardware_encode_codecs: Vec::new(),
                notes: vec![
                    "hardware encode codec enumeration requires a newer VideoToolbox API"
                        .to_owned(),
                ],
            },
            self.source(ProbeArea::Media),
        )
    }

    fn power(&self) -> Capability<PowerInfo> {
        run_command("/usr/bin/pmset", &["-g", "batt"])
            .map(|output| parse_pmset_battery(&output))
            .map_or_else(
                |error| Capability::error(error.to_string()),
                |value| {
                    let low_power_mode = run_command("/usr/bin/pmset", &["-g", "custom"])
                        .ok()
                        .and_then(|output| parse_low_power_mode(&output));
                    Capability::available(
                        PowerInfo {
                            source: value.source,
                            battery_percent: value.battery_percent,
                            charging: value.charging,
                            low_power_mode,
                        },
                        self.source(ProbeArea::Power),
                    )
                },
            )
    }

    fn thermal(&self) -> Capability<ThermalInfo> {
        match run_command("/usr/bin/pmset", &["-g", "therm"]) {
            Ok(output)
                if output
                    .lines()
                    .any(|line| line.trim_start().starts_with("Error:")) =>
            {
                Capability::unavailable(output.trim().to_owned())
            }
            Ok(output) => Capability::available(
                ThermalInfo {
                    state: parse_thermal_state(&output),
                    temperature_milli_celsius: None,
                    sensor: None,
                },
                self.source(ProbeArea::Thermal),
            ),
            Err(error) => Capability::unavailable(error.to_string()),
        }
    }

    fn source(&self, area: ProbeArea) -> DataSource {
        let (provider, detail) = match area {
            ProbeArea::System | ProbeArea::Cpu => ("macos-sysctl", "sysctlbyname FFI"),
            ProbeArea::Memory => (
                "macos-memory",
                "sysctlbyname, Mach host statistics, system_profiler",
            ),
            ProbeArea::Storage => ("macos-storage", "getmntinfo"),
            ProbeArea::Runtimes => ("runtime-commands", "fixed version commands without a shell"),
            ProbeArea::Gpu => ("macos-gpu", "system_profiler JSON"),
            ProbeArea::Media => ("macos-media", "VideoToolbox FFI"),
            ProbeArea::Power => ("macos-power", "pmset"),
            ProbeArea::Thermal => ("macos-thermal", "pmset thermal state"),
        };

        DataSource {
            provider: provider.to_owned(),
            detail: Some(detail.to_owned()),
        }
    }
}

const fn fourcc(value: [u8; 4]) -> u32 {
    ((value[0] as u32) << 24)
        | ((value[1] as u32) << 16)
        | ((value[2] as u32) << 8)
        | value[3] as u32
}

fn run_command(executable: &str, arguments: &[&str]) -> io::Result<String> {
    let output = Command::new(executable).args(arguments).output()?;
    if !output.status.success() {
        let message = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        return Err(io::Error::other(if message.is_empty() {
            format!("{executable} exited with {}", output.status)
        } else {
            message
        }));
    }
    String::from_utf8(output.stdout)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

fn parse_gpu_json(input: &str) -> io::Result<Vec<GpuInfo>> {
    let value: serde_json::Value = serde_json::from_str(input)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let entries = value
        .get("SPDisplaysDataType")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing SPDisplaysDataType"))?;
    Ok(entries
        .iter()
        .filter_map(|entry| {
            let name = entry
                .get("sppci_model")
                .or_else(|| entry.get("_name"))
                .and_then(serde_json::Value::as_str)?;
            let vendor = entry
                .get("spdisplays_vendor")
                .and_then(serde_json::Value::as_str)
                .map(clean_system_profiler_value);
            let mut apis = Vec::new();
            if entry.get("spdisplays_metal").is_some() {
                apis.push("Metal".to_owned());
            }
            Some(GpuInfo {
                name: name.to_owned(),
                vendor,
                driver_version: entry
                    .get("spdisplays_gmux-version")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned),
                memory_bytes: None,
                apis,
            })
        })
        .collect())
}

fn clean_system_profiler_value(value: &str) -> String {
    value
        .strip_prefix("sppci_vendor_")
        .unwrap_or(value)
        .to_owned()
}

fn memory_modules_capability(probe: &NativeProbe) -> Capability<Vec<MemoryModuleInfo>> {
    run_command("/usr/sbin/system_profiler", &["-json", "SPMemoryDataType"])
        .and_then(|output| parse_memory_json(&output))
        .map_or_else(
            |error| Capability::unavailable(error.to_string()),
            |modules| {
                if modules.is_empty() {
                    Capability::unavailable("system_profiler returned no memory modules")
                } else {
                    Capability::available(modules, probe.source(ProbeArea::Memory))
                }
            },
        )
}

fn parse_memory_json(input: &str) -> io::Result<Vec<MemoryModuleInfo>> {
    let value: serde_json::Value = serde_json::from_str(input)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let entries = value
        .get("SPMemoryDataType")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing SPMemoryDataType"))?;

    let mut modules = Vec::new();
    for entry in entries {
        if let Some(module) = parse_memory_entry(entry) {
            modules.push(module);
        }
        if let Some(nested) = entry.get("_items").and_then(serde_json::Value::as_array) {
            for item in nested {
                if let Some(module) = parse_memory_entry(item) {
                    modules.push(module);
                }
            }
        }
    }
    Ok(modules)
}

fn parse_memory_entry(entry: &serde_json::Value) -> Option<MemoryModuleInfo> {
    let slot = entry
        .get("_name")
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned);
    let mem_type = entry
        .get("dimm_type")
        .or_else(|| entry.get("type"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned);
    let speed_mts = entry
        .get("dimm_speed")
        .or_else(|| entry.get("speed"))
        .and_then(serde_json::Value::as_str)
        .and_then(parse_speed_mts);
    let size_bytes = entry
        .get("dimm_size")
        .or_else(|| entry.get("size"))
        .and_then(serde_json::Value::as_str)
        .and_then(parse_memory_size_bytes);
    if mem_type.is_none() && speed_mts.is_none() && size_bytes.is_none() {
        return None;
    }
    Some(MemoryModuleInfo {
        slot,
        mem_type,
        speed_mts,
        size_bytes,
        manufacturer: entry
            .get("dimm_manufacturer")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned),
    })
}

fn parse_speed_mts(input: &str) -> Option<u32> {
    input
        .split_whitespace()
        .next()
        .and_then(|value| value.parse::<u32>().ok())
}

fn parse_memory_size_bytes(input: &str) -> Option<u64> {
    let mut parts = input.split_whitespace();
    let amount = parts.next()?.parse::<f64>().ok()?;
    let unit = parts.next().unwrap_or("GB");
    let multiplier = match unit.to_ascii_uppercase().as_str() {
        "KB" => 1_024.0,
        "MB" => 1_024.0 * 1_024.0,
        "GB" => 1_024.0 * 1_024.0 * 1_024.0,
        "TB" => 1_024.0 * 1_024.0 * 1_024.0 * 1_024.0,
        _ => return None,
    };
    Some((amount * multiplier) as u64)
}

fn parse_pmset_battery(input: &str) -> PowerInfo {
    let source = if input.contains("AC Power") {
        "ac"
    } else if input.contains("Battery Power") {
        "battery"
    } else {
        "unknown"
    };
    let percent = input
        .split_whitespace()
        .find_map(|part| part.strip_suffix("%;"))
        .and_then(|value| value.parse::<u8>().ok());
    let charging = if input.contains("discharging") || input.contains("charged") {
        Some(false)
    } else if input.contains("charging") {
        Some(true)
    } else {
        None
    };
    PowerInfo {
        source: source.to_owned(),
        battery_percent: percent,
        charging,
        low_power_mode: None,
    }
}

fn parse_low_power_mode(input: &str) -> Option<bool> {
    input.lines().find_map(|line| {
        let line = line.trim();
        line.strip_prefix("lowpowermode")
            .and_then(|value| value.trim().parse::<u8>().ok())
            .map(|value| value != 0)
    })
}

fn parse_thermal_state(input: &str) -> String {
    let normalized = input.to_ascii_lowercase();
    if normalized.contains("critical") {
        "critical".to_owned()
    } else if normalized.contains("warning level") && !normalized.contains("no thermal warning") {
        "warning".to_owned()
    } else {
        "normal".to_owned()
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

#[cfg(test)]
mod advanced_tests {
    use super::*;

    #[test]
    fn battery_parser_preserves_source_charge_and_percentage() {
        let input = "Now drawing from 'AC Power'\n -InternalBattery-0\t88%; charging;";

        let power = parse_pmset_battery(input);

        assert_eq!(power.source, "ac");
        assert_eq!(power.battery_percent, Some(88));
        assert_eq!(power.charging, Some(true));
    }

    #[test]
    fn battery_parser_does_not_treat_discharging_as_charging() {
        let input = "Now drawing from 'Battery Power'\n -InternalBattery-0\t72%; discharging;";

        let power = parse_pmset_battery(input);

        assert_eq!(power.source, "battery");
        assert_eq!(power.charging, Some(false));
    }

    #[test]
    fn no_recorded_thermal_warning_is_normal() {
        assert_eq!(
            parse_thermal_state("No thermal warning level has been recorded"),
            "normal"
        );
    }
}
