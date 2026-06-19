mod native;

use mollow_core::{
    Capability, CpuInfo, DataSource, MachineSnapshot, MemoryInfo, PendingCapability,
    SCHEMA_VERSION, SystemInfo,
};

pub use native::NativeProbe;

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

    fn source(&self) -> DataSource;
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
    let source = probe.source();
    let system = observe(probe.system(), &source);
    let cpu = observe(probe.cpu(), &source);
    let memory = observe(probe.memory(), &source);
    let pending = || Capability::<PendingCapability>::unsupported("planned for a future phase");

    MachineSnapshot {
        schema_version: SCHEMA_VERSION.to_owned(),
        mollow_version: mollow_version.to_owned(),
        captured_at_unix_ms,
        system,
        cpu,
        memory,
        storage: pending(),
        gpu: pending(),
        media: pending(),
        power: pending(),
        thermal: pending(),
        runtimes: pending(),
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
    use mollow_core::{CapabilityStatus, CpuInfo, MemoryInfo, SCHEMA_VERSION, SystemInfo};

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
            })
        }

        fn memory(&self) -> Result<MemoryInfo, ProbeError> {
            Err(ProbeError::new("memory", "fixture failure"))
        }

        fn source(&self) -> DataSource {
            DataSource {
                provider: "fixture".to_owned(),
                detail: Some("contract test".to_owned()),
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
        assert_eq!(snapshot.gpu.status, CapabilityStatus::Unsupported);
    }
}
