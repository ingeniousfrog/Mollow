use std::fs;
use std::path::Path;

use mollow_core::BenchmarkRun;
use serde::{Deserialize, Serialize};

const INDEX_FILE: &str = "index.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchiveEntry {
    pub id: String,
    pub stored_path: String,
    pub started_at_unix_ms: u64,
    pub profile: String,
    pub hostname: Option<String>,
    pub build_profile: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchiveIndex {
    pub schema_version: String,
    pub entries: Vec<ArchiveEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrendPoint {
    pub id: String,
    pub started_at_unix_ms: u64,
    pub median_rate_per_second: Option<u64>,
    pub status: String,
}

#[derive(Debug)]
pub struct ArchiveError {
    pub message: String,
}

impl std::fmt::Display for ArchiveError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}", self.message)
    }
}

impl std::error::Error for ArchiveError {}

/// Adds a benchmark run to a local archive directory.
///
/// # Errors
///
/// Returns [`ArchiveError`] when the archive directory or input file cannot be read or written.
pub fn add_run(archive_dir: &Path, input: &Path) -> Result<ArchiveEntry, ArchiveError> {
    fs::create_dir_all(archive_dir).map_err(|error| ArchiveError {
        message: error.to_string(),
    })?;
    let run = read_run(input)?;
    let id = format!("{}-{}", run.started_at_unix_ms, run.profile_to_string());
    let stored_name = format!("{id}.json");
    let stored_path = archive_dir.join(&stored_name);
    fs::copy(input, &stored_path).map_err(|error| ArchiveError {
        message: error.to_string(),
    })?;

    let entry = ArchiveEntry {
        id: id.clone(),
        stored_path: stored_name,
        started_at_unix_ms: run.started_at_unix_ms,
        profile: run.profile_to_string(),
        hostname: run
            .context
            .machine_snapshot
            .system
            .value
            .as_ref()
            .and_then(|system| system.hostname.clone()),
        build_profile: run.context.build_profile.clone(),
    };

    let mut index = load_index(archive_dir)?;
    index.entries.retain(|existing| existing.id != entry.id);
    index.entries.push(entry.clone());
    index.entries.sort_by_key(|entry| entry.started_at_unix_ms);
    save_index(archive_dir, &index)?;
    Ok(entry)
}

/// Lists archived benchmark runs.
///
/// # Errors
///
/// Returns [`ArchiveError`] when the archive index cannot be read.
pub fn list_runs(archive_dir: &Path) -> Result<Vec<ArchiveEntry>, ArchiveError> {
    Ok(load_index(archive_dir)?.entries)
}

/// Builds a trend series for a workload from archived benchmark runs.
///
/// # Errors
///
/// Returns [`ArchiveError`] when archived runs cannot be read.
pub fn trend(archive_dir: &Path, workload: &str) -> Result<Vec<TrendPoint>, ArchiveError> {
    let index = load_index(archive_dir)?;
    let mut points = Vec::new();
    for entry in index.entries {
        let path = archive_dir.join(&entry.stored_path);
        let run = read_run(&path)?;
        let capability = match workload {
            "cpu" => &run.cpu,
            "memory" => &run.memory,
            "storage" => &run.storage,
            "gpu" => &run.gpu,
            "media" => &run.media,
            other => {
                return Err(ArchiveError {
                    message: format!("unknown workload: {other}"),
                });
            }
        };
        points.push(TrendPoint {
            id: entry.id,
            started_at_unix_ms: entry.started_at_unix_ms,
            median_rate_per_second: capability
                .value
                .as_ref()
                .map(|value| value.summary.median_rate_per_second),
            status: format!("{:?}", capability.status).to_ascii_lowercase(),
        });
    }
    Ok(points)
}

fn load_index(archive_dir: &Path) -> Result<ArchiveIndex, ArchiveError> {
    let path = archive_dir.join(INDEX_FILE);
    if !path.exists() {
        return Ok(ArchiveIndex {
            schema_version: "1.0.0".to_owned(),
            entries: Vec::new(),
        });
    }
    let content = fs::read_to_string(path).map_err(|error| ArchiveError {
        message: error.to_string(),
    })?;
    serde_json::from_str(&content).map_err(|error| ArchiveError {
        message: error.to_string(),
    })
}

fn save_index(archive_dir: &Path, index: &ArchiveIndex) -> Result<(), ArchiveError> {
    let content = serde_json::to_string_pretty(index).map_err(|error| ArchiveError {
        message: error.to_string(),
    })?;
    fs::write(archive_dir.join(INDEX_FILE), format!("{content}\n")).map_err(|error| ArchiveError {
        message: error.to_string(),
    })
}

fn read_run(path: &Path) -> Result<BenchmarkRun, ArchiveError> {
    let content = fs::read_to_string(path).map_err(|error| ArchiveError {
        message: error.to_string(),
    })?;
    serde_json::from_str(&content).map_err(|error| ArchiveError {
        message: error.to_string(),
    })
}

trait ProfileName {
    fn profile_to_string(&self) -> String;
}

impl ProfileName for BenchmarkRun {
    fn profile_to_string(&self) -> String {
        match self.profile {
            mollow_core::BenchmarkProfile::Quick => "quick".to_owned(),
            mollow_core::BenchmarkProfile::Standard => "standard".to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    static SEQUENCE: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn add_and_list_archive_entries() {
        let archive_dir = temporary_dir("archive");
        let input = archive_dir.join("input.json");
        fs::write(&input, fixture_run_json(100)).expect("fixture should write");

        let entry = add_run(&archive_dir, &input).expect("archive add should succeed");
        let entries = list_runs(&archive_dir).expect("archive list should succeed");

        assert_eq!(entry.started_at_unix_ms, 100);
        assert_eq!(entries.len(), 1);
    }

    fn temporary_dir(name: &str) -> PathBuf {
        let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "mollow-archive-{name}-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("temporary archive dir should be created");
        path
    }

    fn fixture_run_json(started_at_unix_ms: u64) -> String {
        format!(
            r#"{{
  "schema_version": "4.0.0",
  "mollow_version": "0.1.0",
  "started_at_unix_ms": {started_at_unix_ms},
  "profile": "quick",
  "context": {{
    "build_profile": "release",
    "machine_snapshot": {{
      "schema_version": "4.0.0",
      "mollow_version": "0.1.0",
      "captured_at_unix_ms": 1,
      "system": {{ "status": "unsupported", "value": null, "source": null, "message": "fixture" }},
      "cpu": {{ "status": "unsupported", "value": null, "source": null, "message": "fixture" }},
      "memory": {{ "status": "unsupported", "value": null, "source": null, "message": "fixture" }},
      "storage": {{ "status": "unsupported", "value": null, "source": null, "message": "fixture" }},
      "gpu": {{ "status": "unsupported", "value": null, "source": null, "message": "fixture" }},
      "media": {{ "status": "unsupported", "value": null, "source": null, "message": "fixture" }},
      "power": {{ "status": "unsupported", "value": null, "source": null, "message": "fixture" }},
      "thermal": {{ "status": "unsupported", "value": null, "source": null, "message": "fixture" }},
      "runtimes": {{ "status": "unsupported", "value": null, "source": null, "message": "fixture" }},
      "hardware_context": {{ "status": "unsupported", "value": null, "source": null, "message": "fixture" }},
      "warnings": []
    }}
  }},
  "cpu": {{ "status": "unsupported", "value": null, "source": null, "message": "fixture" }},
  "memory": {{ "status": "unsupported", "value": null, "source": null, "message": "fixture" }},
  "storage": {{ "status": "unsupported", "value": null, "source": null, "message": "fixture" }},
  "gpu": {{ "status": "unsupported", "value": null, "source": null, "message": "fixture" }},
  "media": {{ "status": "unsupported", "value": null, "source": null, "message": "fixture" }},
  "warnings": []
}}"#
        )
    }
}
