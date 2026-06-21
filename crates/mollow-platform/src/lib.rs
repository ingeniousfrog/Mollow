#[cfg(any(target_os = "linux", test))]
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
mod linux_dmi;
#[cfg(any(target_os = "linux", test))]
mod linux_gpu;
#[cfg(target_os = "linux")]
mod linux_media;
#[cfg(any(target_os = "linux", test))]
mod linux_parse;
mod native;
mod runtimes;

use mollow_core::{
    Capability, CpuInfo, DataSource, GpuInfo, HardwareContext, MachineSnapshot, MediaInfo,
    MemoryInfo, PowerInfo, RuntimeInfo, SCHEMA_VERSION, StorageVolume, SystemInfo, ThermalInfo,
    WatchReading,
};

pub use native::NativeProbe;
use runtimes::detect_runtimes;

#[must_use]
pub fn collect_watch_reading(probe: &impl PlatformProbe, captured_at_unix_ms: u64) -> WatchReading {
    WatchReading {
        captured_at_unix_ms,
        memory: observe(probe.memory(), &probe.source(ProbeArea::Memory)),
        power: probe.power(),
        thermal: probe.thermal(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeArea {
    System,
    Cpu,
    Memory,
    Storage,
    Runtimes,
    Gpu,
    Media,
    Power,
    Thermal,
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

    fn gpu(&self) -> Capability<Vec<GpuInfo>> {
        Capability::unsupported("GPU capability detection is not implemented for this platform")
    }

    fn media(&self) -> Capability<MediaInfo> {
        Capability::unsupported("media capability detection is not implemented for this platform")
    }

    fn power(&self) -> Capability<PowerInfo> {
        Capability::unsupported("power capability detection is not implemented for this platform")
    }

    fn thermal(&self) -> Capability<ThermalInfo> {
        Capability::unsupported("thermal capability detection is not implemented for this platform")
    }

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
    let options = SnapshotOptions::default();
    collect_snapshot_with_options(probe, mollow_version, captured_at_unix_ms, &options)
}

#[derive(Debug, Clone, Default)]
pub struct SnapshotOptions {
    pub enrich: bool,
    pub cpu_workload: Option<mollow_core::WorkloadResult>,
    pub gpu_workload: Option<mollow_core::WorkloadResult>,
}

#[must_use]
pub fn collect_snapshot_with_options(
    probe: &impl PlatformProbe,
    mollow_version: &str,
    captured_at_unix_ms: u64,
    options: &SnapshotOptions,
) -> MachineSnapshot {
    let system = observe(probe.system(), &probe.source(ProbeArea::System));
    let cpu = observe(probe.cpu(), &probe.source(ProbeArea::Cpu));
    let memory = observe(probe.memory(), &probe.source(ProbeArea::Memory));
    let storage = observe(probe.storage(), &probe.source(ProbeArea::Storage));
    let runtimes = observe(probe.runtimes(), &probe.source(ProbeArea::Runtimes));
    let mut snapshot = MachineSnapshot {
        schema_version: SCHEMA_VERSION.to_owned(),
        mollow_version: mollow_version.to_owned(),
        captured_at_unix_ms,
        system,
        cpu,
        memory,
        storage,
        gpu: probe.gpu(),
        media: probe.media(),
        power: probe.power(),
        thermal: probe.thermal(),
        runtimes,
        hardware_context: Capability::unsupported(
            "run inspect with --enrich to look up the offline hardware catalog",
        ),
        warnings: Vec::new(),
    };

    if options.enrich {
        snapshot.hardware_context = enrich_snapshot(&snapshot, options);
    }

    snapshot
}

fn enrich_snapshot(
    snapshot: &MachineSnapshot,
    options: &SnapshotOptions,
) -> Capability<HardwareContext> {
    let cpu_model = snapshot
        .cpu
        .value
        .as_ref()
        .and_then(|cpu| cpu.model.as_deref());
    let gpu_names = snapshot.gpu.value.as_deref().unwrap_or_default();
    let memory_modules = snapshot
        .memory
        .value
        .as_ref()
        .and_then(|memory| memory.modules.value.as_deref());
    let cpu_workload = options.cpu_workload.as_ref();
    let gpu_workload = options.gpu_workload.as_ref();

    match mollow_catalog::enrich(mollow_catalog::EnrichmentInput {
        cpu_model,
        gpu_names,
        memory_modules,
        cpu_workload,
        gpu_workload,
    }) {
        Ok(context) => context,
        Err(error) => Capability::error(error.to_string()),
    }
}

/// Attaches offline catalog enrichment to an existing snapshot.
pub fn apply_hardware_enrichment(
    snapshot: &mut MachineSnapshot,
    cpu_workload: Option<&mollow_core::WorkloadResult>,
    gpu_workload: Option<&mollow_core::WorkloadResult>,
) {
    snapshot.hardware_context = enrich_snapshot(
        snapshot,
        &SnapshotOptions {
            enrich: true,
            cpu_workload: cpu_workload.cloned(),
            gpu_workload: gpu_workload.cloned(),
        },
    );
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
