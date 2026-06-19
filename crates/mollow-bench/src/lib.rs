mod gpu;
mod media;
mod statistics;
mod workloads;

use mollow_core::{
    BENCHMARK_SCHEMA_VERSION, BenchmarkContext, BenchmarkProfile, BenchmarkRun, Capability,
    DataSource, MachineSnapshot, WorkloadResult,
};

#[derive(Debug)]
pub struct BenchmarkError {
    pub workload: &'static str,
    pub message: String,
}

impl BenchmarkError {
    #[must_use]
    pub fn new(workload: &'static str, message: impl Into<String>) -> Self {
        Self {
            workload,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for BenchmarkError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.workload, self.message)
    }
}

impl std::error::Error for BenchmarkError {}

/// Runs the supported benchmark suite.
///
/// # Errors
///
/// Returns [`BenchmarkError`] if a workload cannot allocate its bounded input,
/// access its temporary file, or obtain a valid timing sample.
pub fn run_suite(
    profile: BenchmarkProfile,
    mollow_version: &str,
    started_at_unix_ms: u64,
    machine_snapshot: MachineSnapshot,
) -> Result<BenchmarkRun, BenchmarkError> {
    let observe = |result: Result<WorkloadResult, BenchmarkError>| {
        result.map_or_else(
            |error| Capability::error(error.to_string()),
            |value| {
                Capability::available(
                    value.clone(),
                    DataSource {
                        provider: "mollow-bench".to_owned(),
                        detail: Some(format!(
                            "{} workload version {}",
                            value.workload_id, value.workload_version
                        )),
                    },
                )
            },
        )
    };

    let build_profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    let build_warning = if cfg!(debug_assertions) {
        Some(
            "debug build detected; use a release build for comparable performance baselines"
                .to_owned(),
        )
    } else {
        None
    };
    let cpu = observe(workloads::run_cpu(profile));
    let memory = observe(workloads::run_memory(profile));
    let storage = observe(workloads::run_storage(profile));
    let gpu = observe(workloads::run_gpu(profile));
    let media = observe(workloads::run_media(profile));
    let warnings = std::iter::once(build_warning)
        .flatten()
        .chain(
            [
                variation_warning("cpu", &cpu),
                variation_warning("memory", &memory),
                variation_warning("storage", &storage),
                variation_warning("gpu", &gpu),
                variation_warning("media", &media),
            ]
            .into_iter()
            .flatten(),
        )
        .collect();

    Ok(BenchmarkRun {
        schema_version: BENCHMARK_SCHEMA_VERSION.to_owned(),
        mollow_version: mollow_version.to_owned(),
        started_at_unix_ms,
        profile,
        context: BenchmarkContext {
            build_profile: build_profile.to_owned(),
            machine_snapshot,
        },
        cpu,
        memory,
        storage,
        gpu,
        media,
        warnings,
    })
}

fn variation_warning(name: &str, capability: &Capability<WorkloadResult>) -> Option<String> {
    let variation = capability.value.as_ref()?.summary.variation_basis_points;
    (variation > 500).then(|| {
        format!("{name} sample variation is {variation} basis points; treat this result as noisy")
    })
}

#[cfg(test)]
mod tests {
    use mollow_core::{BenchmarkSample, BenchmarkSummary};

    use super::*;

    #[test]
    fn high_variation_produces_an_explanatory_warning() {
        let capability = Capability::available(
            WorkloadResult {
                workload_id: "fixture".to_owned(),
                workload_version: 1,
                measurement: "units_per_second".to_owned(),
                warmup_iterations: 1,
                parameters: Vec::new(),
                samples: vec![BenchmarkSample {
                    elapsed_ns: 1,
                    work_units: 1,
                    rate_per_second: 1,
                }],
                summary: BenchmarkSummary {
                    median_rate_per_second: 1,
                    median_absolute_deviation: 1,
                    minimum_rate_per_second: 1,
                    maximum_rate_per_second: 1,
                    variation_basis_points: 501,
                },
            },
            DataSource {
                provider: "fixture".to_owned(),
                detail: None,
            },
        );

        assert!(variation_warning("fixture", &capability).is_some());
    }
}
