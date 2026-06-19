use std::process::Command;

#[test]
fn inspect_json_emits_a_versioned_snapshot() {
    let output = Command::new(env!("CARGO_BIN_EXE_mollow"))
        .args(["inspect", "--format", "json"])
        .output()
        .expect("mollow should start");

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert!(stdout.contains("\"schema_version\": \"1.0.0\""));
    assert!(stdout.contains("\"system\""));
    assert!(stdout.contains("\"cpu\""));
    assert!(stdout.contains("\"memory\""));
}
