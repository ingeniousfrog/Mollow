use serde::{Deserialize, Serialize};

pub const SCHEMA_VERSION: &str = "1.0.0";

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
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryInfo {
    pub total_bytes: u64,
    pub available_bytes: Option<u64>,
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
    pub storage: Capability<PendingCapability>,
    pub gpu: Capability<PendingCapability>,
    pub media: Capability<PendingCapability>,
    pub power: Capability<PendingCapability>,
    pub thermal: Capability<PendingCapability>,
    pub runtimes: Capability<PendingCapability>,
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
}
