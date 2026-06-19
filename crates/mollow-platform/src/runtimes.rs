use std::io;
use std::process::Command;

use mollow_core::RuntimeInfo;

use crate::ProbeError;

const RUNTIMES: &[(&str, &str, &[&str])] = &[
    ("rustc", "rustc", &["--version"]),
    ("cargo", "cargo", &["--version"]),
    ("git", "git", &["--version"]),
    ("node", "node", &["--version"]),
    ("python", "python3", &["--version"]),
];

pub(crate) fn detect_runtimes() -> Result<Vec<RuntimeInfo>, ProbeError> {
    RUNTIMES.iter().try_fold(Vec::new(), |mut found, spec| {
        if let Some(runtime) = detect_runtime(spec.0, spec.1, spec.2)? {
            found.push(runtime);
        }
        Ok(found)
    })
}

fn detect_runtime(
    name: &str,
    executable: &str,
    arguments: &[&str],
) -> Result<Option<RuntimeInfo>, ProbeError> {
    let output = match Command::new(executable).args(arguments).output() {
        Ok(output) => output,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(ProbeError::new("runtimes", error.to_string())),
    };

    if !output.status.success() {
        return Ok(None);
    }

    let stdout = String::from_utf8(output.stdout)
        .map_err(|error| ProbeError::new("runtimes", error.to_string()))?;
    let version = stdout
        .lines()
        .next()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .ok_or_else(|| ProbeError::new("runtimes", format!("{name} returned no version")))?;

    Ok(Some(RuntimeInfo {
        name: name.to_owned(),
        version: version.to_owned(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_runtime_is_not_an_error() {
        let result = detect_runtime(
            "missing",
            "mollow-command-that-does-not-exist",
            &["--version"],
        )
        .expect("missing executable should be handled");

        assert_eq!(result, None);
    }

    #[test]
    fn rustc_version_is_collected_without_a_shell() {
        let runtime = detect_runtime("rustc", "rustc", &["--version"])
            .expect("rustc discovery should complete")
            .expect("test toolchain should include rustc");

        assert_eq!(runtime.name, "rustc");
        assert!(runtime.version.starts_with("rustc "));
    }
}
