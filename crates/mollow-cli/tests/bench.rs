use std::process::Command;

#[test]
fn bench_quick_json_emits_versioned_workload_results() {
    let output = Command::new(env!("CARGO_BIN_EXE_mollow"))
        .args(["bench", "--profile", "quick", "--format", "json"])
        .output()
        .expect("mollow should start");

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert!(stdout.contains("\"schema_version\": \"3.0.0\""));
    assert!(stdout.contains("\"workload_id\": \"cpu.fnv1a-stream\""));
    assert!(stdout.contains("\"workload_id\": \"memory.sequential-copy\""));
    assert!(stdout.contains("\"workload_id\": \"storage.sequential-write-read\""));
    assert!(stdout.contains("\"workload_id\": \"gpu.matrix-multiply\""));
    assert!(stdout.contains("\"workload_id\": \"media.frame-bytes-process\""));
    assert!(stdout.contains("\"median_absolute_deviation\""));
    let expected_profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    assert!(stdout.contains(&format!("\"build_profile\": \"{expected_profile}\"")));
    assert!(stdout.contains("\"machine_snapshot\""));
    if cfg!(debug_assertions) {
        assert!(stdout.contains("use a release build"));
    }
}
