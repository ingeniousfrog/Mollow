#[cfg(any(target_os = "linux", test))]
mod linux_parse;
mod native;
mod runtimes;

use mollow_core::{
    Capability, CpuInfo, DataSource, MachineSnapshot, MemoryInfo, PendingCapability, RuntimeInfo,
    SCHEMA_VERSION, StorageVolume, SystemInfo,
};

pub use native::NativeProbe;
use runtimes::detect_runtimes;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeArea {
    System,
    Cpu,
    Memory,
    Storage,
    Runtimes,
}

pub trait PlatformProbe {
    /// Collects operating-system and host identity facts.
    ///
    /// # Errors
    ///
    /// Returns a [`ProbeError`] when the native provider cannot complete the
    /// collection operation.
    fn system(&self) -> Result<SystemInfo, ProbeError>;

    /// Collects processor identity and topology facts.
    ///
    /// # Errors
    ///
    /// Returns a [`ProbeError`] when the native provider cannot complete the
    /// collection operation.
    fn cpu(&self) -> Result<CpuInfo, ProbeError>;

    /// Collects installed and currently available memory facts.
    ///
    /// # Errors
    ///
    /// Returns a [`ProbeError`] when the native provider cannot complete the
    /// collection operation.
    fn memory(&self) -> Result<MemoryInfo, ProbeError>;

    /// Collects mounted storage volumes and their current capacity.
    ///
    /// # Errors
    ///
    /// Returns a [`ProbeError`] when mounted volumes cannot be enumerated.
    fn storage(&self) -> Result<Vec<StorageVolume>, ProbeError>;

    /// Collects versions of key development runtimes available on `PATH`.
    ///
    /// # Errors
    ///
    /// Returns a [`ProbeError`] when runtime discovery cannot be completed.
    fn runtimes(&self) -> Result<Vec<RuntimeInfo>, ProbeError>;

    fn source(&self, area: ProbeArea) -> DataSource;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeError {
    pub operation: &'static str,
    pub message: String,
}

impl ProbeError {
    #[must_use]
    pub fn new(operation: &'static str, message: impl Into<String>) -> Self {
        Self {
            operation,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for ProbeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.operation, self.message)
    }
}

impl std::error::Error for ProbeError {}

#[must_use]
pub fn collect_snapshot(
    probe: &impl PlatformProbe,
    mollow_version: &str,
    captured_at_unix_ms: u64,
) -> MachineSnapshot {
    let system = observe(probe.system(), &probe.source(ProbeArea::System));
    let cpu = observe(probe.cpu(), &probe.source(ProbeArea::Cpu));
    let memory = observe(probe.memory(), &probe.source(ProbeArea::Memory));
    let storage = observe(probe.storage(), &probe.source(ProbeArea::Storage));
    let runtimes = observe(probe.runtimes(), &probe.source(ProbeArea::Runtimes));
    let pending = || Capability::<PendingCapability>::unsupported("planned for a future phase");

    MachineSnapshot {
        schema_version: SCHEMA_VERSION.to_owned(),
        mollow_version: mollow_version.to_owned(),
        captured_at_unix_ms,
        system,
        cpu,
        memory,
        storage,
        gpu: pending(),
        media: pending(),
        power: pending(),
        thermal: pending(),
        runtimes,
        warnings: Vec::new(),
    }
}

fn observe<T>(result: Result<T, ProbeError>, source: &DataSource) -> Capability<T> {
    result.map_or_else(
        |error| Capability::error(error.to_string()),
        |value| Capability::available(value, source.clone()),
    )
}

#[must_use]
pub fn native_probe() -> NativeProbe {
    NativeProbe
}

#[cfg(test)]
mod tests {
    use mollow_core::{
        CapabilityStatus, CpuInfo, MemoryInfo, RuntimeInfo, SCHEMA_VERSION, StorageVolume,
        SystemInfo,
    };

    use super::*;

    struct FixtureProbe;

    impl PlatformProbe for FixtureProbe {
        fn system(&self) -> Result<SystemInfo, ProbeError> {
            Ok(SystemInfo {
                os_name: "FixtureOS".to_owned(),
                os_version: Some("1.0".to_owned()),
                kernel_version: Some("24.0".to_owned()),
                architecture: "fixture64".to_owned(),
                hostname: Some("mollow-test".to_owned()),
            })
        }

        fn cpu(&self) -> Result<CpuInfo, ProbeError> {
            Ok(CpuInfo {
                model: Some("Fixture CPU".to_owned()),
                physical_cores: Some(4),
                logical_cores: 8,
                features: vec!["fixture_simd".to_owned()],
            })
        }

        fn memory(&self) -> Result<MemoryInfo, ProbeError> {
            Err(ProbeError::new("memory", "fixture failure"))
        }

        fn storage(&self) -> Result<Vec<StorageVolume>, ProbeError> {
            Ok(vec![StorageVolume {
                name: Some("fixture".to_owned()),
                mount_point: "/".to_owned(),
                file_system: Some("fixturefs".to_owned()),
                total_bytes: 2048,
                available_bytes: 1024,
                read_only: false,
            }])
        }

        fn runtimes(&self) -> Result<Vec<RuntimeInfo>, ProbeError> {
            Ok(vec![RuntimeInfo {
                name: "rustc".to_owned(),
                version: "1.0.0".to_owned(),
            }])
        }

        fn source(&self, area: ProbeArea) -> DataSource {
            DataSource {
                provider: "fixture".to_owned(),
                detail: Some(format!("{area:?} contract test")),
            }
        }
    }

    #[test]
    fn snapshot_preserves_available_and_failed_capabilities() {
        let snapshot = collect_snapshot(&FixtureProbe, "0.1.0-test", 1234);

        assert_eq!(snapshot.schema_version, SCHEMA_VERSION);
        assert_eq!(snapshot.mollow_version, "0.1.0-test");
        assert_eq!(snapshot.captured_at_unix_ms, 1234);
        assert_eq!(snapshot.system.status, CapabilityStatus::Available);
        assert_eq!(snapshot.cpu.status, CapabilityStatus::Available);
        assert_eq!(snapshot.memory.status, CapabilityStatus::Error);
        assert_eq!(
            snapshot.memory.message.as_deref(),
            Some("memory: fixture failure")
        );
        assert_eq!(snapshot.storage.status, CapabilityStatus::Available);
        assert_eq!(snapshot.runtimes.status, CapabilityStatus::Available);
        assert_eq!(snapshot.gpu.status, CapabilityStatus::Unsupported);
    }
}
