use serde::{Deserialize, Serialize};

pub const SCHEMA_VERSION: &str = "3.0.0";
pub const BENCHMARK_SCHEMA_VERSION: &str = "3.0.0";
pub const COMPARISON_SCHEMA_VERSION: &str = "2.0.0";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityStatus {
    Available,
    Unsupported,
    PermissionDenied,
    Unavailable,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DataSource {
    pub provider: String,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Capability<T> {
    pub status: CapabilityStatus,
    pub value: Option<T>,
    pub source: Option<DataSource>,
    pub message: Option<String>,
}

impl<T> Capability<T> {
    pub fn available(value: T, source: DataSource) -> Self {
        Self {
            status: CapabilityStatus::Available,
            value: Some(value),
            source: Some(source),
            message: None,
        }
    }

    pub fn unsupported(message: impl Into<String>) -> Self {
        Self {
            status: CapabilityStatus::Unsupported,
            value: None,
            source: None,
            message: Some(message.into()),
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            status: CapabilityStatus::Error,
            value: None,
            source: None,
            message: Some(message.into()),
        }
    }

    pub fn permission_denied(message: impl Into<String>) -> Self {
        Self {
            status: CapabilityStatus::PermissionDenied,
            value: None,
            source: None,
            message: Some(message.into()),
        }
    }

    pub fn unavailable(message: impl Into<String>) -> Self {
        Self {
            status: CapabilityStatus::Unavailable,
            value: None,
            source: None,
            message: Some(message.into()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SystemInfo {
    pub os_name: String,
    pub os_version: Option<String>,
    pub kernel_version: Option<String>,
    pub architecture: String,
    pub hostname: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CpuInfo {
    pub model: Option<String>,
    pub physical_cores: Option<u32>,
    pub logical_cores: u32,
    pub features: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryInfo {
    pub total_bytes: u64,
    pub available_bytes: Option<u64>,
    pub swap: Capability<SwapInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SwapInfo {
    pub total_bytes: u64,
    pub used_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StorageVolume {
    pub name: Option<String>,
    pub mount_point: String,
    pub file_system: Option<String>,
    pub total_bytes: u64,
    pub available_bytes: u64,
    pub read_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeInfo {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GpuInfo {
    pub name: String,
    pub vendor: Option<String>,
    pub driver_version: Option<String>,
    pub memory_bytes: Option<u64>,
    pub apis: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaInfo {
    pub backend: String,
    pub hardware_decode_codecs: Vec<String>,
    pub hardware_encode_codecs: Vec<String>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PowerInfo {
    pub source: String,
    pub battery_percent: Option<u8>,
    pub charging: Option<bool>,
    pub low_power_mode: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThermalInfo {
    pub state: String,
    pub temperature_milli_celsius: Option<i64>,
    pub sensor: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WatchField {
    Memory,
    Power,
    Thermal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WatchReading {
    pub captured_at_unix_ms: u64,
    pub memory: Capability<MemoryInfo>,
    pub power: Capability<PowerInfo>,
    pub thermal: Capability<ThermalInfo>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BenchmarkProfile {
    Quick,
    Standard,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BenchmarkParameter {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BenchmarkSample {
    pub elapsed_ns: u64,
    pub work_units: u64,
    pub rate_per_second: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BenchmarkSummary {
    pub median_rate_per_second: u64,
    pub median_absolute_deviation: u64,
    pub minimum_rate_per_second: u64,
    pub maximum_rate_per_second: u64,
    pub variation_basis_points: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkloadResult {
    pub workload_id: String,
    pub workload_version: u32,
    pub measurement: String,
    pub warmup_iterations: u32,
    pub parameters: Vec<BenchmarkParameter>,
    pub samples: Vec<BenchmarkSample>,
    pub summary: BenchmarkSummary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BenchmarkContext {
    pub build_profile: String,
    pub machine_snapshot: MachineSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BenchmarkRun {
    pub schema_version: String,
    pub mollow_version: String,
    pub started_at_unix_ms: u64,
    pub profile: BenchmarkProfile,
    pub context: BenchmarkContext,
    pub cpu: Capability<WorkloadResult>,
    pub memory: Capability<WorkloadResult>,
    pub storage: Capability<WorkloadResult>,
    pub gpu: Capability<WorkloadResult>,
    pub media: Capability<WorkloadResult>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeClassification {
    Improvement,
    Regression,
    Stable,
    NotComparable,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkloadComparison {
    pub classification: ChangeClassification,
    pub baseline_rate_per_second: Option<u64>,
    pub candidate_rate_per_second: Option<u64>,
    pub change_basis_points: Option<i32>,
    pub threshold_basis_points: u32,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MachineChange {
    pub field: String,
    pub baseline: Option<String>,
    pub candidate: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComponentChange {
    pub component: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComparisonReport {
    pub schema_version: String,
    pub baseline_started_at_unix_ms: u64,
    pub candidate_started_at_unix_ms: u64,
    pub comparable: bool,
    pub reasons: Vec<String>,
    pub environment_warnings: Vec<String>,
    pub machine_changes: Vec<MachineChange>,
    pub cpu: WorkloadComparison,
    pub memory: WorkloadComparison,
    pub storage: WorkloadComparison,
    pub gpu: WorkloadComparison,
    pub media: WorkloadComparison,
    pub component_changes: Vec<ComponentChange>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingCapability {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MachineSnapshot {
    pub schema_version: String,
    pub mollow_version: String,
    pub captured_at_unix_ms: u64,
    pub system: Capability<SystemInfo>,
    pub cpu: Capability<CpuInfo>,
    pub memory: Capability<MemoryInfo>,
    pub storage: Capability<Vec<StorageVolume>>,
    pub gpu: Capability<Vec<GpuInfo>>,
    pub media: Capability<MediaInfo>,
    pub power: Capability<PowerInfo>,
    pub thermal: Capability<ThermalInfo>,
    pub runtimes: Capability<Vec<RuntimeInfo>>,
    pub warnings: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn available_capability_contains_value_and_source() {
        let source = DataSource {
            provider: "test".to_owned(),
            detail: Some("fixture".to_owned()),
        };

        let capability = Capability::available(42_u8, source.clone());

        assert_eq!(
            capability,
            Capability {
                status: CapabilityStatus::Available,
                value: Some(42),
                source: Some(source),
                message: None,
            }
        );
    }

    #[test]
    fn unsupported_capability_explains_missing_value() {
        let capability = Capability::<u8>::unsupported("not implemented");

        assert_eq!(
            capability,
            Capability {
                status: CapabilityStatus::Unsupported,
                value: None,
                source: None,
                message: Some("not implemented".to_owned()),
            }
        );
    }

    #[test]
    fn permission_denied_capability_preserves_the_reason() {
        let capability = Capability::<u8>::permission_denied("sandbox restriction");

        assert_eq!(capability.status, CapabilityStatus::PermissionDenied);
        assert_eq!(capability.message.as_deref(), Some("sandbox restriction"));
    }
}
