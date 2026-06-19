use std::fs;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[test]
fn capture_compare_and_render_workflow_is_complete() {
    let baseline = temporary_path("baseline.json");
    let candidate = temporary_path("candidate.json");

    run(&[
        "capture",
        "--profile",
        "quick",
        "--format",
        "json",
        "--output",
        baseline.to_str().expect("UTF-8 path"),
    ]);
    run(&[
        "capture",
        "--profile",
        "quick",
        "--format",
        "json",
        "--output",
        candidate.to_str().expect("UTF-8 path"),
    ]);

    let comparison = Command::new(env!("CARGO_BIN_EXE_mollow"))
        .args([
            "compare",
            baseline.to_str().expect("UTF-8 path"),
            candidate.to_str().expect("UTF-8 path"),
            "--format",
            "json",
        ])
        .output()
        .expect("compare should start");
    assert!(comparison.status.success());
    let comparison_body = String::from_utf8_lossy(&comparison.stdout);
    if cfg!(debug_assertions) {
        assert!(comparison_body.contains("\"comparable\": false"));
    } else {
        assert!(comparison_body.contains("\"comparable\": true"));
    }

    let report = Command::new(env!("CARGO_BIN_EXE_mollow"))
        .args([
            "report",
            baseline.to_str().expect("UTF-8 path"),
            "--format",
            "markdown",
            "--lang",
            "zh-CN",
        ])
        .output()
        .expect("report should start");
    assert!(report.status.success());
    assert!(String::from_utf8_lossy(&report.stdout).contains("# 性能基线"));

    fs::remove_file(baseline).expect("baseline cleanup");
    fs::remove_file(candidate).expect("candidate cleanup");
}

fn run(arguments: &[&str]) {
    let status = Command::new(env!("CARGO_BIN_EXE_mollow"))
        .args(arguments)
        .status()
        .expect("mollow should start");
    assert!(status.success());
}

fn temporary_path(suffix: &str) -> std::path::PathBuf {
    let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "mollow-workflow-{}-{sequence}-{suffix}",
        std::process::id()
    ))
}
