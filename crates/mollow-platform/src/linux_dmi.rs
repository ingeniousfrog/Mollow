use std::fs;
use std::path::{Path, PathBuf};

use mollow_core::MemoryModuleInfo;

const DMI_MEMORY_ENTRIES: &str = "/sys/firmware/dmi/entries";

pub(crate) fn read_memory_modules() -> Result<Vec<MemoryModuleInfo>, String> {
    let root = Path::new(DMI_MEMORY_ENTRIES);
    if !root.is_dir() {
        return Err("dmi entries are unavailable".to_owned());
    }

    let mut modules = Vec::new();
    let mut entries = fs::read_dir(root)
        .map_err(|error| error.to_string())?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("17-"))
        })
        .collect::<Vec<PathBuf>>();
    entries.sort();

    for entry in entries {
        let size_kb = read_dmi_u64(&entry.join("size")).unwrap_or(0);
        if size_kb == 0 {
            continue;
        }
        modules.push(MemoryModuleInfo {
            slot: read_dmi_string(&entry.join("locator")),
            mem_type: read_dmi_string(&entry.join("type")).map(|value| normalize_dmi_type(&value)),
            speed_mts: read_dmi_u64(&entry.join("speed"))
                .and_then(|value| u32::try_from(value).ok()),
            size_bytes: size_kb.checked_mul(1024),
            manufacturer: read_dmi_string(&entry.join("manufacturer")),
        });
    }

    if modules.is_empty() {
        Err("no populated memory modules were found in dmi".to_owned())
    } else {
        Ok(modules)
    }
}

fn read_dmi_string(path: &Path) -> Option<String> {
    let value = fs::read_to_string(path).ok()?;
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("unknown") {
        None
    } else {
        Some(trimmed.to_owned())
    }
}

fn read_dmi_u64(path: &Path) -> Option<u64> {
    let value = fs::read_to_string(path).ok()?;
    value.trim().parse::<u64>().ok()
}

fn normalize_dmi_type(input: &str) -> String {
    if input.chars().all(|character| character.is_ascii_digit()) {
        dmi_type_code(input).unwrap_or_else(|| input.to_owned())
    } else {
        input.to_owned()
    }
}

fn dmi_type_code(code: &str) -> Option<String> {
    match code {
        "26" => Some("DDR4".to_owned()),
        "34" => Some("DDR5".to_owned()),
        "29" => Some("LPDDR5".to_owned()),
        "30" => Some("LPDDR5X".to_owned()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_dmi_type_maps_ddr_codes() {
        assert_eq!(normalize_dmi_type("26"), "DDR4");
        assert_eq!(normalize_dmi_type("DDR5"), "DDR5");
    }
}
