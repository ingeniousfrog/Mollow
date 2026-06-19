use mollow_core::MachineSnapshot;

#[derive(Debug)]
pub enum ReportError {
    Serialization(serde_json::Error),
}

impl std::fmt::Display for ReportError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Serialization(error) => write!(formatter, "failed to serialize report: {error}"),
        }
    }
}

impl std::error::Error for ReportError {}

/// Renders a machine snapshot as stable, pretty-printed JSON.
///
/// # Errors
///
/// Returns [`ReportError::Serialization`] if the snapshot cannot be encoded.
pub fn render_json(snapshot: &MachineSnapshot) -> Result<String, ReportError> {
    let mut report = serde_json::to_string_pretty(snapshot).map_err(ReportError::Serialization)?;
    report.push('\n');
    Ok(report)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use mollow_core::{
        Capability, CpuInfo, DataSource, MachineSnapshot, MemoryInfo, SCHEMA_VERSION, SystemInfo,
    };

    use super::*;

    #[test]
    fn json_report_is_pretty_and_versioned() {
        let source = DataSource {
            provider: "fixture".to_owned(),
            detail: None,
        };
        let snapshot = MachineSnapshot {
            schema_version: SCHEMA_VERSION.to_owned(),
            mollow_version: "0.1.0".to_owned(),
            captured_at_unix_ms: 1234,
            system: Capability::available(
                SystemInfo {
                    os_name: "FixtureOS".to_owned(),
                    os_version: None,
                    kernel_version: None,
                    architecture: "fixture64".to_owned(),
                    hostname: None,
                },
                source.clone(),
            ),
            cpu: Capability::available(
                CpuInfo {
                    model: None,
                    physical_cores: None,
                    logical_cores: 2,
                    features: vec!["fixture_simd".to_owned()],
                },
                source.clone(),
            ),
            memory: Capability::available(
                MemoryInfo {
                    total_bytes: 1024,
                    available_bytes: None,
                    swap: Capability::unsupported("fixture"),
                },
                source,
            ),
            storage: Capability::available(
                Vec::new(),
                DataSource {
                    provider: "fixture".to_owned(),
                    detail: None,
                },
            ),
            gpu: Capability::unsupported("future phase"),
            media: Capability::unsupported("future phase"),
            power: Capability::unsupported("future phase"),
            thermal: Capability::unsupported("future phase"),
            runtimes: Capability::available(
                Vec::new(),
                DataSource {
                    provider: "fixture".to_owned(),
                    detail: None,
                },
            ),
            warnings: Vec::new(),
        };

        let report = render_json(&snapshot).expect("snapshot should serialize");

        assert!(report.contains("\"schema_version\": \"2.0.0\""));
        assert!(report.ends_with('\n'));
    }

    #[test]
    fn bundled_schema_matches_the_snapshot_schema_version() {
        let schema_directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../schemas");
        let schema_path = schema_directory.join("machine-snapshot-v2.schema.json");
        let schema = fs::read_to_string(schema_path).expect("snapshot schema should exist");
        let schema: serde_json::Value =
            serde_json::from_str(&schema).expect("snapshot schema should be valid JSON");

        assert_eq!(
            schema["properties"]["schema_version"]["const"],
            SCHEMA_VERSION
        );
        assert_eq!(
            schema["properties"]["storage"]["$ref"],
            "#/$defs/storageCapability"
        );
        assert_eq!(
            schema["properties"]["runtimes"]["$ref"],
            "#/$defs/runtimeCapability"
        );
        assert_eq!(
            schema["$defs"]["memoryInfo"]["properties"]["swap"]["$ref"],
            "#/$defs/swapCapability"
        );
        assert_eq!(
            schema["$defs"]["cpuInfo"]["properties"]["features"]["type"],
            "array"
        );

        let legacy_schema =
            fs::read_to_string(schema_directory.join("machine-snapshot-v1.schema.json"))
                .expect("legacy snapshot schema should remain available");
        let legacy_schema: serde_json::Value =
            serde_json::from_str(&legacy_schema).expect("legacy schema should be valid JSON");
        assert_eq!(
            legacy_schema["properties"]["schema_version"]["const"],
            "1.0.0"
        );
    }
}
