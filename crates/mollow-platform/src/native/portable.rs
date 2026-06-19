use mollow_core::{CpuInfo, DataSource, MemoryInfo, RuntimeInfo, StorageVolume, SystemInfo};

use crate::detect_runtimes;
use crate::{PlatformProbe, ProbeArea, ProbeError};

pub struct NativeProbe;

impl PlatformProbe for NativeProbe {
    fn system(&self) -> Result<SystemInfo, ProbeError> {
        Ok(SystemInfo {
            os_name: std::env::consts::OS.to_owned(),
            os_version: None,
            kernel_version: None,
            architecture: std::env::consts::ARCH.to_owned(),
            hostname: None,
        })
    }

    fn cpu(&self) -> Result<CpuInfo, ProbeError> {
        let logical_cores = std::thread::available_parallelism()
            .map_err(|error| ProbeError::new("cpu", error.to_string()))?
            .get();

        Ok(CpuInfo {
            model: None,
            physical_cores: None,
            logical_cores: u32::try_from(logical_cores)
                .map_err(|error| ProbeError::new("cpu", error.to_string()))?,
            features: Vec::new(),
        })
    }

    fn storage(&self) -> Result<Vec<StorageVolume>, ProbeError> {
        Err(ProbeError::new(
            "storage",
            "native storage probe is not implemented for this platform",
        ))
    }

    fn runtimes(&self) -> Result<Vec<RuntimeInfo>, ProbeError> {
        detect_runtimes()
    }

    fn memory(&self) -> Result<MemoryInfo, ProbeError> {
        Err(ProbeError::new(
            "memory",
            "native memory probe is not implemented for this platform",
        ))
    }

    fn source(&self, area: ProbeArea) -> DataSource {
        DataSource {
            provider: "portable-rust".to_owned(),
            detail: Some(format!("{area:?} standard library fallback")),
        }
    }
}
