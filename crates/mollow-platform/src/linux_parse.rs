use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct ParsedCpu {
    pub model: Option<String>,
    pub physical_cores: Option<u32>,
    pub features: Vec<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct ParsedMemory {
    pub total: u64,
    pub available: Option<u64>,
    pub swap_total: u64,
    pub swap_used: u64,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct ParsedMount {
    pub source: String,
    pub mount_point: String,
    pub file_system: String,
    pub read_only: bool,
}

pub(crate) fn parse_cpuinfo(input: &str) -> ParsedCpu {
    let records = input
        .split("\n\n")
        .map(parse_key_value_lines)
        .filter(|record| !record.is_empty())
        .collect::<Vec<_>>();
    let model = records.iter().find_map(|record| {
        ["model name", "Hardware", "Processor"]
            .into_iter()
            .find_map(|key| record.get(key).cloned())
    });
    let physical_cores = records
        .iter()
        .filter_map(|record| {
            Some((
                record.get("physical id")?.clone(),
                record.get("core id")?.clone(),
            ))
        })
        .collect::<BTreeSet<_>>();
    let physical_cores = (!physical_cores.is_empty())
        .then(|| u32::try_from(physical_cores.len()).ok())
        .flatten();
    let features = records
        .iter()
        .find_map(|record| record.get("flags").or_else(|| record.get("Features")))
        .map_or_else(Vec::new, |features| {
            features
                .split_whitespace()
                .map(str::to_owned)
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect()
        });

    ParsedCpu {
        model,
        physical_cores,
        features,
    }
}

pub(crate) fn parse_meminfo(input: &str) -> Result<ParsedMemory, String> {
    let values = parse_key_value_lines(input);
    let bytes = |key: &str| -> Result<u64, String> {
        values
            .get(key)
            .ok_or_else(|| format!("missing {key}"))?
            .split_whitespace()
            .next()
            .ok_or_else(|| format!("missing value for {key}"))?
            .parse::<u64>()
            .map_err(|error| format!("invalid {key}: {error}"))?
            .checked_mul(1024)
            .ok_or_else(|| format!("{key} overflowed"))
    };
    let total_bytes = bytes("MemTotal")?;
    let available_bytes = bytes("MemAvailable").ok();
    let swap_total_bytes = bytes("SwapTotal")?;
    let swap_free_bytes = bytes("SwapFree")?;

    Ok(ParsedMemory {
        total: total_bytes,
        available: available_bytes,
        swap_total: swap_total_bytes,
        swap_used: swap_total_bytes.saturating_sub(swap_free_bytes),
    })
}

pub(crate) fn parse_os_release(input: &str) -> (String, Option<String>) {
    let values = input
        .lines()
        .filter_map(|line| line.split_once('='))
        .map(|(key, value)| (key, value.trim_matches('"').to_owned()))
        .collect::<BTreeMap<_, _>>();
    let name = values
        .get("PRETTY_NAME")
        .or_else(|| values.get("NAME"))
        .cloned()
        .unwrap_or_else(|| "Linux".to_owned());

    (name, values.get("VERSION_ID").cloned())
}

pub(crate) fn parse_mountinfo(input: &str) -> Result<Vec<ParsedMount>, String> {
    input
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let (mount, file_system) = line
                .split_once(" - ")
                .ok_or_else(|| "mountinfo line is missing separator".to_owned())?;
            let mount_fields = mount.split_whitespace().collect::<Vec<_>>();
            let file_system_fields = file_system.split_whitespace().collect::<Vec<_>>();
            let mount_point = mount_fields
                .get(4)
                .ok_or_else(|| "mountinfo line is missing mount point".to_owned())?;
            let options = mount_fields
                .get(5)
                .ok_or_else(|| "mountinfo line is missing mount options".to_owned())?;
            let file_system = file_system_fields
                .first()
                .ok_or_else(|| "mountinfo line is missing file system".to_owned())?;
            let source = file_system_fields
                .get(1)
                .ok_or_else(|| "mountinfo line is missing source".to_owned())?;

            Ok(ParsedMount {
                source: decode_mount_field(source)?,
                mount_point: decode_mount_field(mount_point)?,
                file_system: (*file_system).to_owned(),
                read_only: options.split(',').any(|option| option == "ro"),
            })
        })
        .collect()
}

fn parse_key_value_lines(input: &str) -> BTreeMap<&str, String> {
    input
        .lines()
        .filter_map(|line| line.split_once(':'))
        .map(|(key, value)| (key.trim(), value.trim().to_owned()))
        .collect()
}

fn decode_mount_field(input: &str) -> Result<String, String> {
    let bytes = input.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] == b'\\' && index + 3 < bytes.len() {
            let digits = &bytes[index + 1..=index + 3];
            if digits.iter().all(|digit| (b'0'..=b'7').contains(digit)) {
                let value = (digits[0] - b'0') * 64 + (digits[1] - b'0') * 8 + (digits[2] - b'0');
                output.push(value);
                index += 4;
                continue;
            }
        }
        output.push(bytes[index]);
        index += 1;
    }

    String::from_utf8(output).map_err(|error| format!("invalid mount field: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpuinfo_collects_model_topology_and_sorted_features() {
        let input = "\
processor : 0
physical id : 0
core id : 0
model name : Fixture CPU
flags : sse2 avx aes

processor : 1
physical id : 0
core id : 1
model name : Fixture CPU
flags : sse2 avx aes
";

        assert_eq!(
            parse_cpuinfo(input),
            ParsedCpu {
                model: Some("Fixture CPU".to_owned()),
                physical_cores: Some(2),
                features: vec!["aes".to_owned(), "avx".to_owned(), "sse2".to_owned()],
            }
        );
    }

    #[test]
    fn meminfo_converts_kibibytes_and_derives_used_swap() {
        let input = "\
MemTotal:       16384 kB
MemAvailable:    4096 kB
SwapTotal:       2048 kB
SwapFree:         512 kB
";

        assert_eq!(
            parse_meminfo(input).expect("fixture should parse"),
            ParsedMemory {
                total: 16_777_216,
                available: Some(4_194_304),
                swap_total: 2_097_152,
                swap_used: 1_572_864,
            }
        );
    }

    #[test]
    fn os_release_prefers_pretty_name_and_keeps_version() {
        let input = "NAME=Fixture\nVERSION_ID=\"24.04\"\nPRETTY_NAME=\"Fixture Linux 24.04\"\n";

        assert_eq!(
            parse_os_release(input),
            ("Fixture Linux 24.04".to_owned(), Some("24.04".to_owned()))
        );
    }

    #[test]
    fn mountinfo_decodes_paths_and_read_only_state() {
        let input = "\
36 25 0:32 / / rw,relatime - ext4 /dev/vda1 rw
37 25 0:33 / /media/My\\040Disk ro,nosuid - exfat /dev/sdb1 ro
";

        assert_eq!(
            parse_mountinfo(input).expect("fixture should parse"),
            vec![
                ParsedMount {
                    source: "/dev/vda1".to_owned(),
                    mount_point: "/".to_owned(),
                    file_system: "ext4".to_owned(),
                    read_only: false,
                },
                ParsedMount {
                    source: "/dev/sdb1".to_owned(),
                    mount_point: "/media/My Disk".to_owned(),
                    file_system: "exfat".to_owned(),
                    read_only: true,
                },
            ]
        );
    }
}
