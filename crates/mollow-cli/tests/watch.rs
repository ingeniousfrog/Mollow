use std::process::Command;

#[test]
fn watch_runs_for_a_single_refresh_cycle() {
    let output = Command::new(env!("CARGO_BIN_EXE_mollow"))
        .args(["watch", "-i", "1", "--count", "1", "--lang", "english"])
        .output()
        .expect("mollow should start");

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert!(stdout.contains("Mollow Watch"));
    assert!(stdout.contains("Updated:"));
    assert!(stdout.contains('-'));
    assert!(stdout.contains(':'));
    assert!(stdout.contains("Memory available / total"));
    assert!(stdout.contains("Power"));
    assert!(stdout.contains("Thermal"));
}

#[test]
fn watch_rejects_zero_interval() {
    let output = Command::new(env!("CARGO_BIN_EXE_mollow"))
        .args(["watch", "-i", "0", "--count", "1"])
        .output()
        .expect("mollow should start");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert!(stderr.contains("interval must be at least 1 second"));
}
