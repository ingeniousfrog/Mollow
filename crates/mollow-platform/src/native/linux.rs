use std::ffi::{CStr, CString};
use std::fs;
use std::io;
use std::mem::MaybeUninit;
use std::path::Path;

use mollow_core::{
    Capability, CpuInfo, DataSource, GpuInfo, MediaInfo, MemoryInfo, MemoryModuleInfo, PowerInfo,
    RuntimeInfo, StorageVolume, SwapInfo, SystemInfo, ThermalInfo,
};

#[cfg(target_os = "linux")]
use crate::linux_gpu;
#[cfg(target_os = "linux")]
use crate::linux_media::{v4l2_codecs, vaapi_codecs};
#[cfg(target_os = "linux")]
use crate::linux_dmi;
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
            modules: memory_modules_capability(self),
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

    fn gpu(&self) -> Capability<Vec<GpuInfo>> {
        linux_gpu::enumerate_gpus().map_or_else(
            |error| Capability::error(error.to_string()),
            |gpus| {
                if gpus.is_empty() {
                    Capability::unavailable("no DRM GPU devices were found")
                } else {
                    Capability::available(gpus, self.source(ProbeArea::Gpu))
                }
            },
        )
    }

    fn media(&self) -> Capability<MediaInfo> {
        let mut decode_codecs = vaapi_codecs().unwrap_or_default();
        decode_codecs.extend(v4l2_codecs().unwrap_or_default());
        decode_codecs.sort();
        decode_codecs.dedup();

        if decode_codecs.is_empty() {
            let render_nodes = fs::read_dir("/dev/dri")
                .ok()
                .into_iter()
                .flatten()
                .filter_map(Result::ok)
                .any(|entry| entry.file_name().to_string_lossy().starts_with("renderD"));
            if render_nodes {
                return Capability::unavailable(
                    "DRM render node is present but VA-API/V4L2 codecs were not enumerated",
                );
            }
            return Capability::unavailable("no DRM render node was found");
        }

        Capability::available(
            MediaInfo {
                backend: "VA-API/V4L2".to_owned(),
                hardware_decode_codecs: decode_codecs,
                hardware_encode_codecs: Vec::new(),
                notes: Vec::new(),
            },
            self.source(ProbeArea::Media),
        )
    }

    fn power(&self) -> Capability<PowerInfo> {
        linux_power().map_or_else(
            |error| Capability::unavailable(error.to_string()),
            |power| Capability::available(power, self.source(ProbeArea::Power)),
        )
    }

    fn thermal(&self) -> Capability<ThermalInfo> {
        linux_thermal().map_or_else(
            |error| Capability::unavailable(error.to_string()),
            |thermal| Capability::available(thermal, self.source(ProbeArea::Thermal)),
        )
    }

    fn source(&self, area: ProbeArea) -> DataSource {
        let (provider, detail) = match area {
            ProbeArea::System => ("linux-system", "/etc/os-release and uname"),
            ProbeArea::Cpu => ("linux-cpu", "/proc/cpuinfo"),
            ProbeArea::Memory => ("linux-memory", "/proc/meminfo and DMI type 17"),
            ProbeArea::Storage => ("linux-storage", "/proc/self/mountinfo and statvfs"),
            ProbeArea::Runtimes => ("runtime-commands", "fixed version commands without a shell"),
            ProbeArea::Gpu => ("linux-gpu", "DRM sysfs, nvidia-smi, pci.ids"),
            ProbeArea::Media => ("linux-media", "VA-API and V4L2"),
            ProbeArea::Power => ("linux-power", "power_supply sysfs"),
            ProbeArea::Thermal => ("linux-thermal", "thermal sysfs"),
        };

        DataSource {
            provider: provider.to_owned(),
            detail: Some(detail.to_owned()),
        }
    }
}

#[cfg(target_os = "linux")]
fn memory_modules_capability(probe: &NativeProbe) -> Capability<Vec<MemoryModuleInfo>> {
    match linux_dmi::read_memory_modules() {
        Ok(modules) => Capability::available(modules, probe.source(ProbeArea::Memory)),
        Err(message) if message.contains("unavailable") => Capability::unavailable(message),
        Err(message) => Capability::error(message),
    }
}

#[cfg(not(target_os = "linux"))]
fn memory_modules_capability(_probe: &NativeProbe) -> Capability<Vec<MemoryModuleInfo>> {
    Capability::unsupported("memory module details are not implemented for this platform")
}

fn linux_power() -> io::Result<PowerInfo> {
    let entries = fs::read_dir("/sys/class/power_supply")?;
    let mut ac_online = false;
    let mut battery = None;
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        let supply_type = read_trimmed(path.join("type")).unwrap_or_default();
        if supply_type == "Mains" || supply_type == "USB" {
            ac_online |= read_trimmed(path.join("online")).is_ok_and(|value| value == "1");
        }
        if supply_type == "Battery" && battery.is_none() {
            let percent = read_trimmed(path.join("capacity"))
                .ok()
                .and_then(|value| value.parse::<u8>().ok());
            let status = read_trimmed(path.join("status")).unwrap_or_default();
            battery = Some((percent, status));
        }
    }
    let (battery_percent, status) =
        battery.ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no battery found"))?;
    Ok(PowerInfo {
        source: if ac_online { "ac" } else { "battery" }.to_owned(),
        battery_percent,
        charging: match status.as_str() {
            "Charging" => Some(true),
            "Discharging" | "Full" => Some(false),
            _ => None,
        },
        low_power_mode: None,
    })
}

fn linux_thermal() -> io::Result<ThermalInfo> {
    let entries = fs::read_dir("/sys/class/thermal")?;
    let mut hottest: Option<(i64, String)> = None;
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if !entry
            .file_name()
            .to_string_lossy()
            .starts_with("thermal_zone")
        {
            continue;
        }
        let Ok(value) = read_trimmed(path.join("temp")).and_then(|value| {
            value
                .parse::<i64>()
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
        }) else {
            continue;
        };
        let sensor = read_trimmed(path.join("type")).unwrap_or_else(|_| "unknown".to_owned());
        if hottest.as_ref().is_none_or(|current| value > current.0) {
            hottest = Some((value, sensor));
        }
    }
    let (temperature, sensor) = hottest
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no thermal sensors found"))?;
    Ok(ThermalInfo {
        state: if temperature >= 90_000 {
            "critical"
        } else if temperature >= 80_000 {
            "warning"
        } else {
            "normal"
        }
        .to_owned(),
        temperature_milli_celsius: Some(temperature),
        sensor: Some(sensor),
    })
}

fn read_trimmed(path: impl AsRef<Path>) -> io::Result<String> {
    Ok(fs::read_to_string(path)?.trim().to_owned())
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
