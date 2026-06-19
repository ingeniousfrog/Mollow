use std::collections::BTreeMap;

use mollow_core::{
    BenchmarkRun, COMPARISON_SCHEMA_VERSION, Capability, ChangeClassification, ComparisonReport,
    ComponentChange, MachineChange, MachineSnapshot, WorkloadComparison, WorkloadResult,
};

const SIGNIFICANT_CHANGE_BASIS_POINTS: u32 = 500;

/// Compares two complete benchmark runs.
///
/// # Errors
///
/// Returns a message when either run violates invariants required by the
/// comparison model.
pub fn compare_runs(
    baseline: &BenchmarkRun,
    candidate: &BenchmarkRun,
) -> Result<ComparisonReport, String> {
    validate_run(baseline, "baseline")?;
    validate_run(candidate, "candidate")?;

    let mut reasons = Vec::new();
    if baseline.schema_version != candidate.schema_version {
        reasons.push(format!(
            "benchmark schema differs: {} vs {}",
            baseline.schema_version, candidate.schema_version
        ));
    }
    if baseline.profile != candidate.profile {
        reasons.push(format!(
            "benchmark profile differs: {:?} vs {:?}",
            baseline.profile, candidate.profile
        ));
    }
    if baseline.context.build_profile != candidate.context.build_profile {
        reasons.push(format!(
            "build profile differs: {} vs {}",
            baseline.context.build_profile, candidate.context.build_profile
        ));
    }
    if baseline.context.build_profile != "release" || candidate.context.build_profile != "release" {
        reasons.push("release builds are required for comparable performance baselines".to_owned());
    }
    let comparable = reasons.is_empty();
    let environment_warnings = environment_warnings(
        &baseline.context.machine_snapshot,
        &candidate.context.machine_snapshot,
    );

    Ok(ComparisonReport {
        schema_version: COMPARISON_SCHEMA_VERSION.to_owned(),
        baseline_started_at_unix_ms: baseline.started_at_unix_ms,
        candidate_started_at_unix_ms: candidate.started_at_unix_ms,
        comparable,
        reasons: reasons.clone(),
        environment_warnings,
        machine_changes: machine_changes(
            &baseline.context.machine_snapshot,
            &candidate.context.machine_snapshot,
        ),
        cpu: compare_workload(&baseline.cpu, &candidate.cpu, &reasons),
        memory: compare_workload(&baseline.memory, &candidate.memory, &reasons),
        storage: compare_workload(&baseline.storage, &candidate.storage, &reasons),
        gpu: compare_workload(&baseline.gpu, &candidate.gpu, &reasons),
        media: compare_workload(&baseline.media, &candidate.media, &reasons),
        component_changes: component_changes(baseline, candidate),
    })
}

/// Compares two machine snapshots without workload performance deltas.
#[must_use]
pub fn compare_snapshots(
    baseline: &MachineSnapshot,
    candidate: &MachineSnapshot,
) -> ComparisonReport {
    let environment_warnings = environment_warnings(baseline, candidate);
    ComparisonReport {
        schema_version: COMPARISON_SCHEMA_VERSION.to_owned(),
        baseline_started_at_unix_ms: baseline.captured_at_unix_ms,
        candidate_started_at_unix_ms: candidate.captured_at_unix_ms,
        comparable: false,
        reasons: vec!["snapshot comparison does not include benchmark workload results".to_owned()],
        environment_warnings,
        machine_changes: machine_changes(baseline, candidate),
        cpu: unavailable(vec!["snapshot comparison only".to_owned()]),
        memory: unavailable(vec!["snapshot comparison only".to_owned()]),
        storage: unavailable(vec!["snapshot comparison only".to_owned()]),
        gpu: unavailable(vec!["snapshot comparison only".to_owned()]),
        media: unavailable(vec!["snapshot comparison only".to_owned()]),
        component_changes: Vec::new(),
    }
}

fn validate_run(run: &BenchmarkRun, label: &str) -> Result<(), String> {
    if run.schema_version.is_empty() {
        return Err(format!("{label} schema version is empty"));
    }
    if run.started_at_unix_ms == 0 {
        return Err(format!("{label} timestamp must be non-zero"));
    }
    Ok(())
}

fn compare_workload(
    baseline: &Capability<WorkloadResult>,
    candidate: &Capability<WorkloadResult>,
    global_reasons: &[String],
) -> WorkloadComparison {
    let mut reasons = global_reasons.to_vec();
    let Some(baseline) = baseline.value.as_ref() else {
        reasons.push("baseline workload is unavailable".to_owned());
        return unavailable(reasons);
    };
    let Some(candidate) = candidate.value.as_ref() else {
        reasons.push("candidate workload is unavailable".to_owned());
        return unavailable(reasons);
    };

    if baseline.workload_id != candidate.workload_id {
        reasons.push(format!(
            "workload id differs: {} vs {}",
            baseline.workload_id, candidate.workload_id
        ));
    }
    if baseline.workload_version != candidate.workload_version {
        reasons.push(format!(
            "workload version differs: {} vs {}",
            baseline.workload_version, candidate.workload_version
        ));
    }
    if baseline.measurement != candidate.measurement {
        reasons.push(format!(
            "measurement differs: {} vs {}",
            baseline.measurement, candidate.measurement
        ));
    }
    if baseline.parameters != candidate.parameters {
        reasons.push("workload parameters differ".to_owned());
    }

    let baseline_rate = baseline.summary.median_rate_per_second;
    let candidate_rate = candidate.summary.median_rate_per_second;
    if !reasons.is_empty() {
        return WorkloadComparison {
            classification: ChangeClassification::NotComparable,
            baseline_rate_per_second: Some(baseline_rate),
            candidate_rate_per_second: Some(candidate_rate),
            change_basis_points: None,
            threshold_basis_points: SIGNIFICANT_CHANGE_BASIS_POINTS,
            reasons,
        };
    }
    if baseline_rate == 0 {
        return unavailable(vec!["baseline median rate is zero".to_owned()]);
    }

    let difference = i128::from(candidate_rate) - i128::from(baseline_rate);
    let change = difference
        .saturating_mul(10_000)
        .checked_div(i128::from(baseline_rate))
        .unwrap_or(0)
        .clamp(i128::from(i32::MIN), i128::from(i32::MAX));
    let change = i32::try_from(change).unwrap_or(if change.is_negative() {
        i32::MIN
    } else {
        i32::MAX
    });
    let threshold = i32::try_from(SIGNIFICANT_CHANGE_BASIS_POINTS).unwrap_or(i32::MAX);
    let classification = if change <= -threshold {
        ChangeClassification::Regression
    } else if change >= threshold {
        ChangeClassification::Improvement
    } else {
        ChangeClassification::Stable
    };

    WorkloadComparison {
        classification,
        baseline_rate_per_second: Some(baseline_rate),
        candidate_rate_per_second: Some(candidate_rate),
        change_basis_points: Some(change),
        threshold_basis_points: SIGNIFICANT_CHANGE_BASIS_POINTS,
        reasons,
    }
}

fn unavailable(reasons: Vec<String>) -> WorkloadComparison {
    WorkloadComparison {
        classification: ChangeClassification::Unavailable,
        baseline_rate_per_second: None,
        candidate_rate_per_second: None,
        change_basis_points: None,
        threshold_basis_points: SIGNIFICANT_CHANGE_BASIS_POINTS,
        reasons,
    }
}

fn machine_changes(baseline: &MachineSnapshot, candidate: &MachineSnapshot) -> Vec<MachineChange> {
    let mut changes = Vec::new();
    append_system_changes(&mut changes, baseline, candidate);
    append_resource_changes(&mut changes, baseline, candidate);
    append_advanced_changes(&mut changes, baseline, candidate);
    append_runtime_changes(&mut changes, baseline, candidate);
    changes.sort_by(|left, right| left.field.cmp(&right.field));
    changes.dedup_by(|left, right| left.field == right.field);
    changes
}

fn append_system_changes(
    changes: &mut Vec<MachineChange>,
    baseline: &MachineSnapshot,
    candidate: &MachineSnapshot,
) {
    push_change(
        changes,
        "system.os",
        baseline
            .system
            .value
            .as_ref()
            .map(|value| format!("{} {}", value.os_name, optional(value.os_version.as_ref()))),
        candidate
            .system
            .value
            .as_ref()
            .map(|value| format!("{} {}", value.os_name, optional(value.os_version.as_ref()))),
    );
    push_change(
        changes,
        "cpu.model",
        baseline
            .cpu
            .value
            .as_ref()
            .and_then(|value| value.model.clone()),
        candidate
            .cpu
            .value
            .as_ref()
            .and_then(|value| value.model.clone()),
    );
    push_change(
        changes,
        "cpu.logical_cores",
        baseline
            .cpu
            .value
            .as_ref()
            .map(|value| value.logical_cores.to_string()),
        candidate
            .cpu
            .value
            .as_ref()
            .map(|value| value.logical_cores.to_string()),
    );
}

fn append_resource_changes(
    changes: &mut Vec<MachineChange>,
    baseline: &MachineSnapshot,
    candidate: &MachineSnapshot,
) {
    push_change(
        changes,
        "memory.total_bytes",
        baseline
            .memory
            .value
            .as_ref()
            .map(|value| value.total_bytes.to_string()),
        candidate
            .memory
            .value
            .as_ref()
            .map(|value| value.total_bytes.to_string()),
    );
    push_change(
        changes,
        "memory.available_bytes",
        baseline
            .memory
            .value
            .as_ref()
            .and_then(|value| value.available_bytes)
            .map(|value| value.to_string()),
        candidate
            .memory
            .value
            .as_ref()
            .and_then(|value| value.available_bytes)
            .map(|value| value.to_string()),
    );
}

fn append_advanced_changes(
    changes: &mut Vec<MachineChange>,
    baseline: &MachineSnapshot,
    candidate: &MachineSnapshot,
) {
    push_change(
        changes,
        "gpu",
        gpu_summary(baseline),
        gpu_summary(candidate),
    );
    push_change(
        changes,
        "power.source",
        baseline
            .power
            .value
            .as_ref()
            .map(|value| value.source.clone()),
        candidate
            .power
            .value
            .as_ref()
            .map(|value| value.source.clone()),
    );
    push_change(
        changes,
        "power.low_power_mode",
        baseline
            .power
            .value
            .as_ref()
            .and_then(|value| value.low_power_mode)
            .map(|value| value.to_string()),
        candidate
            .power
            .value
            .as_ref()
            .and_then(|value| value.low_power_mode)
            .map(|value| value.to_string()),
    );
    push_change(
        changes,
        "thermal.state",
        baseline
            .thermal
            .value
            .as_ref()
            .map(|value| value.state.clone()),
        candidate
            .thermal
            .value
            .as_ref()
            .map(|value| value.state.clone()),
    );
}

fn append_runtime_changes(
    changes: &mut Vec<MachineChange>,
    baseline: &MachineSnapshot,
    candidate: &MachineSnapshot,
) {
    let baseline_runtimes = runtime_map(baseline);
    let candidate_runtimes = runtime_map(candidate);
    for name in baseline_runtimes.keys().chain(candidate_runtimes.keys()) {
        push_change(
            changes,
            &format!("runtime.{name}"),
            baseline_runtimes.get(name).cloned(),
            candidate_runtimes.get(name).cloned(),
        );
    }
}

fn gpu_summary(snapshot: &MachineSnapshot) -> Option<String> {
    snapshot.gpu.value.as_ref().map(|gpus| {
        gpus.iter()
            .map(|gpu| format!("{} [{}]", gpu.name, gpu.apis.join(",")))
            .collect::<Vec<_>>()
            .join("; ")
    })
}

fn runtime_map(snapshot: &MachineSnapshot) -> BTreeMap<String, String> {
    snapshot
        .runtimes
        .value
        .as_deref()
        .unwrap_or_default()
        .iter()
        .map(|runtime| (runtime.name.clone(), runtime.version.clone()))
        .collect()
}

fn optional(value: Option<&String>) -> &str {
    value.map_or("unknown", String::as_str)
}

fn push_change(
    changes: &mut Vec<MachineChange>,
    field: &str,
    baseline: Option<String>,
    candidate: Option<String>,
) {
    if baseline != candidate {
        changes.push(MachineChange {
            field: field.to_owned(),
            baseline,
            candidate,
        });
    }
}

fn component_changes(baseline: &BenchmarkRun, candidate: &BenchmarkRun) -> Vec<ComponentChange> {
    [
        ("gpu", &baseline.gpu.status, &candidate.gpu.status),
        ("media", &baseline.media.status, &candidate.media.status),
    ]
    .into_iter()
    .filter(|(_, baseline, candidate)| baseline != candidate)
    .map(|(component, baseline, candidate)| ComponentChange {
        component: component.to_owned(),
        message: format!("status changed from {baseline:?} to {candidate:?}"),
    })
    .collect()
}

fn environment_warnings(baseline: &MachineSnapshot, candidate: &MachineSnapshot) -> Vec<String> {
    let mut warnings = Vec::new();
    if power_source(baseline) != power_source(candidate) {
        warnings.push(format!(
            "power source changed from {} to {}",
            power_source(baseline).unwrap_or("unknown"),
            power_source(candidate).unwrap_or("unknown")
        ));
    }
    if low_power_mode(baseline) != low_power_mode(candidate) {
        warnings.push("low power mode changed between captures".to_owned());
    }
    if thermal_state(baseline) != thermal_state(candidate) {
        warnings.push(format!(
            "thermal state changed from {} to {}",
            thermal_state(baseline).unwrap_or("unknown"),
            thermal_state(candidate).unwrap_or("unknown")
        ));
    }
    warnings
}

fn power_source(snapshot: &MachineSnapshot) -> Option<&str> {
    snapshot
        .power
        .value
        .as_ref()
        .map(|value| value.source.as_str())
}

fn low_power_mode(snapshot: &MachineSnapshot) -> Option<bool> {
    snapshot
        .power
        .value
        .as_ref()
        .and_then(|value| value.low_power_mode)
}

fn thermal_state(snapshot: &MachineSnapshot) -> Option<&str> {
    snapshot
        .thermal
        .value
        .as_ref()
        .map(|value| value.state.as_str())
}

#[cfg(test)]
mod tests {
    use mollow_core::{
        BenchmarkContext, BenchmarkProfile, BenchmarkSample, BenchmarkSummary, Capability,
        DataSource, MachineSnapshot, WorkloadResult,
    };

    use super::*;

    #[test]
    fn matching_runs_classify_a_significant_regression() {
        let baseline = fixture_run(1000, BenchmarkProfile::Quick, "release");
        let candidate = fixture_run(890, BenchmarkProfile::Quick, "release");

        let report = compare_runs(&baseline, &candidate).expect("runs should compare");

        assert!(report.comparable);
        assert_eq!(
            report.cpu.classification,
            mollow_core::ChangeClassification::Regression
        );
        assert_eq!(report.cpu.change_basis_points, Some(-1100));
    }

    #[test]
    fn profile_mismatch_prevents_performance_comparison() {
        let baseline = fixture_run(1000, BenchmarkProfile::Quick, "release");
        let candidate = fixture_run(1100, BenchmarkProfile::Standard, "release");

        let report = compare_runs(&baseline, &candidate).expect("runs should compare");

        assert!(!report.comparable);
        assert!(
            report
                .reasons
                .iter()
                .any(|reason| reason.contains("profile"))
        );
    }

    #[test]
    fn workload_parameter_mismatch_is_explicitly_not_comparable() {
        let baseline = fixture_run(1000, BenchmarkProfile::Quick, "release");
        let candidate =
            fixture_run_with_parameter(1100, BenchmarkProfile::Quick, "release", "different");

        let report = compare_runs(&baseline, &candidate).expect("runs should compare");

        assert_eq!(
            report.cpu.classification,
            mollow_core::ChangeClassification::NotComparable
        );
    }

    fn fixture_run(rate: u64, profile: BenchmarkProfile, build_profile: &str) -> BenchmarkRun {
        fixture_run_with_parameter(rate, profile, build_profile, "1024")
    }

    fn fixture_run_with_parameter(
        rate: u64,
        profile: BenchmarkProfile,
        build_profile: &str,
        parameter: &str,
    ) -> BenchmarkRun {
        let source = DataSource {
            provider: "fixture".to_owned(),
            detail: None,
        };
        let workload = WorkloadResult {
            workload_id: "fixture".to_owned(),
            workload_version: 1,
            measurement: "bytes_per_second".to_owned(),
            warmup_iterations: 1,
            parameters: vec![mollow_core::BenchmarkParameter {
                name: "size".to_owned(),
                value: parameter.to_owned(),
            }],
            samples: vec![BenchmarkSample {
                elapsed_ns: 1,
                work_units: 1,
                rate_per_second: rate,
            }],
            summary: BenchmarkSummary {
                median_rate_per_second: rate,
                median_absolute_deviation: 0,
                minimum_rate_per_second: rate,
                maximum_rate_per_second: rate,
                variation_basis_points: 0,
            },
        };
        let capability = || Capability::available(workload.clone(), source.clone());
        BenchmarkRun {
            schema_version: mollow_core::BENCHMARK_SCHEMA_VERSION.to_owned(),
            mollow_version: "0.1.0".to_owned(),
            started_at_unix_ms: 1,
            profile,
            context: BenchmarkContext {
                build_profile: build_profile.to_owned(),
                machine_snapshot: MachineSnapshot {
                    schema_version: "3.0.0".to_owned(),
                    mollow_version: "0.1.0".to_owned(),
                    captured_at_unix_ms: 1,
                    system: Capability::unsupported("fixture"),
                    cpu: Capability::unsupported("fixture"),
                    memory: Capability::unsupported("fixture"),
                    storage: Capability::unsupported("fixture"),
                    gpu: Capability::unsupported("fixture"),
                    media: Capability::unsupported("fixture"),
                    power: Capability::unsupported("fixture"),
                    thermal: Capability::unsupported("fixture"),
                    runtimes: Capability::unsupported("fixture"),
                    warnings: Vec::new(),
                },
            },
            cpu: capability(),
            memory: capability(),
            storage: capability(),
            gpu: capability(),
            media: capability(),
            warnings: Vec::new(),
        }
    }
}
