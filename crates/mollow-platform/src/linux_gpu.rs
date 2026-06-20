#![cfg_attr(not(target_os = "linux"), allow(dead_code))]

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use mollow_core::GpuInfo;

const PCI_IDS_PATHS: &[&str] = &[
    "/usr/share/misc/pci.ids",
    "/usr/share/hwdata/pci.ids",
    "/var/lib/pciutils/pci.ids",
];

static PCI_IDS_CACHE: OnceLock<PciIdsDatabase> = OnceLock::new();

#[derive(Debug, Clone, PartialEq, Eq)]
struct DrmGpuRecord {
    pci_slot: Option<String>,
    vendor_id: Option<String>,
    device_id: Option<String>,
    vendor: Option<String>,
    driver_version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NvidiaSmiRecord {
    name: String,
    pci_bus_id: String,
    driver_version: String,
    memory_mib: Option<u64>,
}

#[derive(Debug, Default, Clone)]
struct PciIdsDatabase {
    vendors: HashMap<String, String>,
    devices: HashMap<(String, String), String>,
}

pub(crate) fn enumerate_gpus() -> io::Result<Vec<GpuInfo>> {
    let records = dedupe_drm_records(enumerate_drm_records()?);
    let nvidia_smi = query_nvidia_smi().unwrap_or_default();
    let pci_ids = load_pci_ids();

    Ok(records
        .into_iter()
        .map(|record| resolve_gpu(record, &nvidia_smi, &pci_ids))
        .collect())
}

fn enumerate_drm_records() -> io::Result<Vec<DrmGpuRecord>> {
    let entries = fs::read_dir("/sys/class/drm")?;
    let mut records = Vec::new();

    for entry in entries.filter_map(Result::ok) {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !is_drm_card(&name) {
            continue;
        }
        let device = entry.path().join("device");
        if !device.exists() {
            continue;
        }

        let vendor_id = read_trimmed(device.join("vendor")).ok();
        let device_id = read_trimmed(device.join("device")).ok();
        let vendor = vendor_id.as_deref().map(pci_vendor_name);
        let pci_slot = read_pci_slot(&device);
        let driver = fs::read_link(device.join("driver")).ok().and_then(|path| {
            path.file_name()
                .map(|name| name.to_string_lossy().into_owned())
        });
        let driver_version = driver.as_ref().and_then(|driver| {
            read_trimmed(Path::new("/sys/module").join(driver).join("version")).ok()
        });

        records.push(DrmGpuRecord {
            pci_slot,
            vendor_id,
            device_id,
            vendor,
            driver_version,
        });
    }

    Ok(records)
}

fn dedupe_drm_records(records: Vec<DrmGpuRecord>) -> Vec<DrmGpuRecord> {
    let mut seen = HashSet::new();
    records
        .into_iter()
        .filter(|record| {
            let key = record
                .pci_slot
                .as_deref()
                .map(normalize_pci_bus_id)
                .or_else(|| {
                    record
                        .vendor_id
                        .as_ref()
                        .zip(record.device_id.as_ref())
                        .map(|(vendor, device)| {
                            format!("{}:{}", normalize_pci_id(vendor), normalize_pci_id(device))
                        })
                });
            key.is_none_or(|key| seen.insert(key))
        })
        .collect()
}

fn resolve_gpu(
    record: DrmGpuRecord,
    nvidia_smi: &BTreeMap<String, NvidiaSmiRecord>,
    pci_ids: &PciIdsDatabase,
) -> GpuInfo {
    let mut apis = vec!["DRM".to_owned()];
    let vendor_id = record.vendor_id.as_deref().map(normalize_pci_id);
    let device_id = record.device_id.as_deref().map(normalize_pci_id);

    if vendor_id.as_deref() == Some("10de") {
        if let Some(slot) = record.pci_slot.as_deref() {
            let normalized = normalize_pci_bus_id(slot);
            if let Some(smi) = nvidia_smi.get(&normalized) {
                apis.push("NVIDIA-SMI".to_owned());
                return GpuInfo {
                    name: smi.name.clone(),
                    vendor: record.vendor,
                    driver_version: Some(smi.driver_version.clone()),
                    memory_bytes: smi.memory_mib.and_then(|mib| mib.checked_mul(1024 * 1024)),
                    apis,
                };
            }
        }
    }

    if let (Some(vendor_id), Some(device_id)) = (&vendor_id, &device_id) {
        if let Some(name) = pci_ids.lookup(vendor_id, device_id) {
            return GpuInfo {
                name,
                vendor: record.vendor,
                driver_version: record.driver_version,
                memory_bytes: None,
                apis,
            };
        }
    }

    if let Some(slot) = record.pci_slot.as_deref() {
        if let Some(name) = query_lspci_name(slot) {
            return GpuInfo {
                name,
                vendor: record.vendor,
                driver_version: record.driver_version,
                memory_bytes: None,
                apis,
            };
        }
    }

    GpuInfo {
        name: fallback_name(&record),
        vendor: record.vendor,
        driver_version: record.driver_version,
        memory_bytes: None,
        apis,
    }
}

fn fallback_name(record: &DrmGpuRecord) -> String {
    format!(
        "{} {}",
        record.vendor.as_deref().unwrap_or("PCI GPU"),
        record.device_id.as_deref().unwrap_or("unknown")
    )
}

fn read_pci_slot(device: &Path) -> Option<String> {
    read_trimmed(device.join("uevent"))
        .ok()
        .and_then(|content| {
            content.lines().find_map(|line| {
                line.strip_prefix("PCI_SLOT_NAME=")
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_owned)
            })
        })
}

fn query_nvidia_smi() -> io::Result<BTreeMap<String, NvidiaSmiRecord>> {
    let output = match Command::new("nvidia-smi")
        .args([
            "--query-gpu=gpu_name,pci.bus_id,driver_version,memory.total",
            "--format=csv,noheader,nounits",
        ])
        .output()
    {
        Ok(output) => output,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(BTreeMap::new());
        }
        Err(error) => return Err(error),
    };

    if !output.status.success() {
        return Ok(BTreeMap::new());
    }

    let stdout = String::from_utf8(output.stdout)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    Ok(parse_nvidia_smi_csv(&stdout))
}

fn parse_nvidia_smi_csv(input: &str) -> BTreeMap<String, NvidiaSmiRecord> {
    input
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() {
                return None;
            }
            let mut fields = line.splitn(4, ',').map(str::trim);
            let name = fields.next()?.to_owned();
            let pci_bus_id = fields.next()?.to_owned();
            let driver_version = fields.next()?.to_owned();
            let memory_mib = fields.next().and_then(|value| value.parse::<u64>().ok());
            let normalized = normalize_pci_bus_id(&pci_bus_id);
            Some((
                normalized,
                NvidiaSmiRecord {
                    name,
                    pci_bus_id,
                    driver_version,
                    memory_mib,
                },
            ))
        })
        .collect()
}

fn load_pci_ids() -> PciIdsDatabase {
    PCI_IDS_CACHE
        .get_or_init(|| {
            PCI_IDS_PATHS
                .iter()
                .find_map(|path| {
                    fs::read_to_string(path)
                        .ok()
                        .map(|content| parse_pci_ids(&content))
                })
                .unwrap_or_default()
        })
        .clone()
}

fn parse_pci_ids(input: &str) -> PciIdsDatabase {
    let mut database = PciIdsDatabase::default();
    let mut current_vendor: Option<String> = None;

    for line in input.lines() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }

        if !line.starts_with('\t') {
            let mut parts = line.split_whitespace();
            let Some(vendor_id) = parts.next() else {
                continue;
            };
            let vendor_name = parts.collect::<Vec<_>>().join(" ");
            if vendor_name.is_empty() {
                continue;
            }
            current_vendor = Some(normalize_pci_id(vendor_id));
            database
                .vendors
                .insert(current_vendor.clone().unwrap_or_default(), vendor_name);
            continue;
        }

        let Some(vendor_id) = current_vendor.as_ref() else {
            continue;
        };
        let trimmed = line.trim_start();
        let mut parts = trimmed.split_whitespace();
        let Some(device_id) = parts.next() else {
            continue;
        };
        let device_name = parts.collect::<Vec<_>>().join(" ");
        if device_name.is_empty() {
            continue;
        }
        database.devices.insert(
            (vendor_id.clone(), normalize_pci_id(device_id)),
            device_name,
        );
    }

    database
}

impl PciIdsDatabase {
    fn lookup(&self, vendor_id: &str, device_id: &str) -> Option<String> {
        let device_name = self
            .devices
            .get(&(vendor_id.to_owned(), device_id.to_owned()))?;
        let vendor_name = self.vendors.get(vendor_id)?;
        Some(format_pci_name(vendor_name, device_name))
    }
}

fn format_pci_name(vendor: &str, device: &str) -> String {
    let vendor_short = vendor
        .trim_end_matches(" Corporation")
        .trim_end_matches(" Corp.")
        .trim_end_matches(" Inc.")
        .trim_end_matches(" Co.")
        .trim();
    let device_lower = device.to_ascii_lowercase();
    let vendor_lower = vendor_short.to_ascii_lowercase();

    if device_lower.starts_with(&vendor_lower) {
        device.to_owned()
    } else {
        format!("{vendor_short} {device}")
    }
}

fn query_lspci_name(pci_slot: &str) -> Option<String> {
    let output = Command::new("lspci")
        .args(["-nn", "-s", pci_slot])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8(output.stdout).ok()?;
    parse_lspci_name(&stdout)
}

fn parse_lspci_name(input: &str) -> Option<String> {
    let line = input.lines().next()?.trim();
    let (_, rest) = line.rsplit_once("]: ")?;
    let mut name = rest.trim();
    if let Some(stripped) = name.strip_suffix(')') {
        if let Some((base, _)) = stripped.rsplit_once(" (rev ") {
            name = base.trim();
        }
    }
    let name = strip_pci_id_suffix(name);
    (!name.is_empty()).then(|| name.to_owned())
}

fn strip_pci_id_suffix(name: &str) -> &str {
    let Some(start) = name.rfind(" [") else {
        return name.trim();
    };
    let bracket = &name[start + 2..name.len().saturating_sub(1)];
    if bracket.len() == 9
        && bracket.as_bytes().get(4) == Some(&b':')
        && bracket[..4].chars().all(|ch| ch.is_ascii_hexdigit())
        && bracket[5..].chars().all(|ch| ch.is_ascii_hexdigit())
    {
        name[..start].trim()
    } else {
        name.trim()
    }
}

pub(crate) fn normalize_pci_bus_id(id: &str) -> String {
    let id = id.trim().to_ascii_lowercase();
    let parts: Vec<&str> = id.split(':').collect();
    let (domain, bus, dev_fn) = match parts.as_slice() {
        [bus, dev_fn] => (0_u32, *bus, *dev_fn),
        [domain, bus, dev_fn] => (parse_hex_u32(domain).unwrap_or(0), *bus, *dev_fn),
        _ => return id,
    };

    let bus = parse_hex_u8(bus);
    let (device, function) = split_dev_fn(dev_fn);
    format!(
        "{:04x}:{:02x}:{:02x}.{}",
        domain & 0xffff,
        bus,
        device,
        function
    )
}

fn normalize_pci_id(id: &str) -> String {
    let id = id.trim().trim_start_matches("0x").to_ascii_lowercase();
    if id.is_empty() {
        return "0000".to_owned();
    }
    format!("{:04x}", parse_hex_u32(&id).unwrap_or(0) & 0xffff)
}

fn split_dev_fn(value: &str) -> (u8, u8) {
    let value = value.trim();
    if let Some((device, function)) = value.split_once('.') {
        (parse_hex_u8(device), parse_hex_u8(function))
    } else {
        (parse_hex_u8(value), 0)
    }
}

fn parse_hex_u32(value: &str) -> Option<u32> {
    let value = value.trim().trim_start_matches("0x");
    if value.is_empty() {
        return Some(0);
    }
    u32::from_str_radix(value, 16).ok()
}

fn parse_hex_u8(value: &str) -> u8 {
    let value = value.trim().trim_start_matches("0x");
    if value.is_empty() {
        return 0;
    }
    u8::from_str_radix(value, 16).unwrap_or(0)
}

fn is_drm_card(name: &str) -> bool {
    name.strip_prefix("card").is_some_and(|suffix| {
        !suffix.is_empty() && suffix.bytes().all(|byte| byte.is_ascii_digit())
    })
}

fn pci_vendor_name(id: &str) -> String {
    match id.trim_start_matches("0x").to_ascii_lowercase().as_str() {
        "1002" => "AMD".to_owned(),
        "10de" => "NVIDIA".to_owned(),
        "8086" => "Intel".to_owned(),
        "106b" => "Apple".to_owned(),
        "1013" => "Cirrus Logic".to_owned(),
        other => format!("PCI {other}"),
    }
}

fn read_trimmed(path: PathBuf) -> io::Result<String> {
    Ok(fs::read_to_string(path)?.trim().to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    const PCI_IDS_FIXTURE: &str = r"
10de  NVIDIA Corporation
	2236  GA102GL [A10 24GB]
1013  Cirrus Logic
	00b8  GD 5446
";

    #[test]
    fn normalize_pci_bus_id_accepts_short_and_long_domain() {
        assert_eq!(
            normalize_pci_bus_id("00000000:00:03.0"),
            normalize_pci_bus_id("0000:00:03.0")
        );
        assert_eq!(normalize_pci_bus_id("0000:00:03.0"), "0000:00:03.0");
    }

    #[test]
    fn parse_pci_ids_resolves_vendor_device_pairs() {
        let database = parse_pci_ids(PCI_IDS_FIXTURE);
        assert_eq!(
            database.lookup("10de", "2236"),
            Some("NVIDIA GA102GL [A10 24GB]".to_owned())
        );
        assert_eq!(
            database.lookup("1013", "00b8"),
            Some("Cirrus Logic GD 5446".to_owned())
        );
    }

    #[test]
    fn parse_nvidia_smi_csv_builds_bus_id_index() {
        let records = parse_nvidia_smi_csv(
            "NVIDIA A10, 00000000:00:03.0, 535.183.01, 23028\nNVIDIA A10, 0000:00:04.0, 535.183.01, 23028\n",
        );
        assert_eq!(records.len(), 2);
        let first = records.get("0000:00:03.0").expect("first gpu");
        assert_eq!(first.name, "NVIDIA A10");
        assert_eq!(first.driver_version, "535.183.01");
        assert_eq!(first.memory_mib, Some(23028));
    }

    #[test]
    fn resolve_gpu_prefers_nvidia_smi_for_nvidia_devices() {
        let record = DrmGpuRecord {
            pci_slot: Some("0000:00:03.0".to_owned()),
            vendor_id: Some("0x10de".to_owned()),
            device_id: Some("0x2236".to_owned()),
            vendor: Some("NVIDIA".to_owned()),
            driver_version: Some("535.104.05".to_owned()),
        };
        let mut smi = parse_nvidia_smi_csv("NVIDIA A10, 00000000:00:03.0, 535.183.01, 23028");
        let pci_ids = parse_pci_ids(PCI_IDS_FIXTURE);
        let gpu = resolve_gpu(record, &smi, &pci_ids);

        assert_eq!(gpu.name, "NVIDIA A10");
        assert_eq!(gpu.driver_version.as_deref(), Some("535.183.01"));
        assert_eq!(gpu.memory_bytes, Some(23028 * 1024 * 1024));
        assert!(gpu.apis.contains(&"NVIDIA-SMI".to_owned()));

        smi.remove("0000:00:03.0");
        let record = DrmGpuRecord {
            pci_slot: Some("0000:00:03.0".to_owned()),
            vendor_id: Some("0x10de".to_owned()),
            device_id: Some("0x2236".to_owned()),
            vendor: Some("NVIDIA".to_owned()),
            driver_version: Some("535.104.05".to_owned()),
        };
        let gpu = resolve_gpu(record, &smi, &pci_ids);
        assert_eq!(gpu.name, "NVIDIA GA102GL [A10 24GB]");
    }

    #[test]
    fn resolve_gpu_uses_pci_ids_for_non_nvidia_devices() {
        let record = DrmGpuRecord {
            pci_slot: Some("0000:00:1f.0".to_owned()),
            vendor_id: Some("0x1013".to_owned()),
            device_id: Some("0x00b8".to_owned()),
            vendor: Some("Cirrus Logic".to_owned()),
            driver_version: None,
        };
        let gpu = resolve_gpu(record, &BTreeMap::new(), &parse_pci_ids(PCI_IDS_FIXTURE));
        assert_eq!(gpu.name, "Cirrus Logic GD 5446");
    }

    #[test]
    fn resolve_gpu_falls_back_to_pci_ids_when_lookup_misses() {
        let record = DrmGpuRecord {
            pci_slot: None,
            vendor_id: Some("0x9999".to_owned()),
            device_id: Some("0xabcd".to_owned()),
            vendor: Some("PCI 9999".to_owned()),
            driver_version: None,
        };
        let gpu = resolve_gpu(record, &BTreeMap::new(), &parse_pci_ids(PCI_IDS_FIXTURE));
        assert_eq!(gpu.name, "PCI 9999 0xabcd");
    }

    #[test]
    fn dedupe_drm_records_keeps_one_entry_per_pci_slot() {
        let records = vec![
            DrmGpuRecord {
                pci_slot: Some("0000:00:03.0".to_owned()),
                vendor_id: Some("0x10de".to_owned()),
                device_id: Some("0x2236".to_owned()),
                vendor: Some("NVIDIA".to_owned()),
                driver_version: None,
            },
            DrmGpuRecord {
                pci_slot: Some("0000:00:03.0".to_owned()),
                vendor_id: Some("0x10de".to_owned()),
                device_id: Some("0x2236".to_owned()),
                vendor: Some("NVIDIA".to_owned()),
                driver_version: Some("535.183.01".to_owned()),
            },
        ];
        let deduped = dedupe_drm_records(records);
        assert_eq!(deduped.len(), 1);
    }

    #[test]
    fn parse_lspci_name_extracts_device_label() {
        assert_eq!(
            parse_lspci_name(
                "00:03.0 VGA compatible controller [0300]: NVIDIA Corporation GA102GL [A10 24GB] [10de:2236] (rev a1)"
            ),
            Some("NVIDIA Corporation GA102GL [A10 24GB]".to_owned())
        );
    }
}
