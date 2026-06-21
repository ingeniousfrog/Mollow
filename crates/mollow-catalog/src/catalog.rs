use mollow_core::{
    BenchmarkReferenceMatch, Capability, CpuCatalogMatch, DataSource, GpuCatalogMatch,
    GpuInfo, HardwareContext, MatchConfidence, MemoryCatalogMatch, MemoryModuleInfo,
    WorkloadResult,
};
use serde::Deserialize;

use crate::normalize::{matches_pattern, normalize_model};

const EMBEDDED_CATALOG: &str = include_str!("../../../data/hardware/catalog.json");

#[derive(Debug, Deserialize)]
struct HardwareCatalog {
    version: String,
    benchmark_reference_version: String,
    score_source: String,
    cpus: Vec<CpuEntry>,
    gpus: Vec<GpuEntry>,
    memory_profiles: Vec<MemoryProfileEntry>,
}

#[derive(Debug, Deserialize)]
struct CpuEntry {
    match_patterns: Vec<String>,
    canonical_model: String,
    codename: Option<String>,
    architecture_family: Option<String>,
    architecture_summary: Option<String>,
    process_nm: Option<u32>,
    base_clock_mhz: Option<u32>,
    boost_clock_mhz: Option<u32>,
    l3_cache_mb: Option<u32>,
    tdp_watts: Option<u32>,
    performance_core_count: Option<u32>,
    efficiency_core_count: Option<u32>,
    reference_urls: Vec<String>,
    diagram_template: Option<String>,
    cpu_reference_score: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct GpuEntry {
    match_patterns: Vec<String>,
    canonical_model: String,
    codename: Option<String>,
    architecture_family: Option<String>,
    architecture_summary: Option<String>,
    process_nm: Option<u32>,
    shader_units: Option<u32>,
    memory_type: Option<String>,
    memory_bus_bits: Option<u32>,
    tdp_watts: Option<u32>,
    reference_urls: Vec<String>,
    diagram_template: Option<String>,
    gpu_reference_score: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct MemoryProfileEntry {
    match_patterns: Vec<String>,
    memory_type: Option<String>,
    speed_mts: Option<u32>,
    channels: Option<u32>,
    architecture_summary: Option<String>,
    reference_urls: Vec<String>,
    diagram_template: Option<String>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct EnrichmentInput<'a> {
    pub cpu_model: Option<&'a str>,
    pub gpu_names: &'a [GpuInfo],
    pub memory_modules: Option<&'a [MemoryModuleInfo]>,
    pub cpu_workload: Option<&'a WorkloadResult>,
    pub gpu_workload: Option<&'a WorkloadResult>,
}

#[derive(Debug)]
pub enum CatalogError {
    Parse(String),
}

impl std::fmt::Display for CatalogError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Parse(message) => write!(formatter, "catalog parse error: {message}"),
        }
    }
}

impl std::error::Error for CatalogError {}

fn load_catalog() -> Result<HardwareCatalog, CatalogError> {
    serde_json::from_str(EMBEDDED_CATALOG)
        .map_err(|error| CatalogError::Parse(error.to_string()))
}

fn catalog_source() -> DataSource {
    DataSource {
        provider: "mollow-catalog".to_owned(),
        detail: Some("embedded offline hardware catalog".to_owned()),
    }
}

/// Looks up hardware specifications and optional benchmark reference context.
///
/// # Errors
///
/// Returns [`CatalogError`] when the embedded catalog cannot be parsed.
pub fn enrich(input: EnrichmentInput<'_>) -> Result<Capability<HardwareContext>, CatalogError> {
    let catalog = load_catalog()?;
    let source = catalog_source();

    let cpu = match input.cpu_model {
        Some(model) => lookup_cpu(&catalog, model),
        None => Capability::unavailable("cpu model is unavailable for catalog lookup"),
    };
    let gpu = lookup_gpus(&catalog, input.gpu_names);
    let memory = lookup_memory(&catalog, input.memory_modules);
    let benchmark_reference =
        build_benchmark_reference(&catalog, &cpu, &gpu, input.cpu_workload, input.gpu_workload);

    let has_match = cpu.value.is_some() || gpu.value.is_some() || memory.value.is_some();
    if !has_match {
        return Ok(Capability::unavailable(
            "no catalog entries matched the detected hardware",
        ));
    }

    Ok(Capability::available(
        HardwareContext {
            catalog_version: catalog.version,
            cpu,
            gpu,
            memory,
            benchmark_reference,
        },
        source,
    ))
}

fn lookup_cpu(catalog: &HardwareCatalog, model: &str) -> Capability<CpuCatalogMatch> {
    let normalized = normalize_model(model);
    let Some(entry) = catalog
        .cpus
        .iter()
        .find(|entry| entry_match(&normalized, &entry.match_patterns))
    else {
        return Capability::unavailable(format!(
            "cpu model {model:?} is not present in catalog {}",
            catalog.version
        ));
    };

    Capability::available(
        CpuCatalogMatch {
            matched_model: entry.canonical_model.clone(),
            confidence: match_confidence(&normalized, &entry.match_patterns),
            codename: entry.codename.clone(),
            architecture_family: entry.architecture_family.clone(),
            architecture_summary: entry.architecture_summary.clone(),
            process_nm: entry.process_nm,
            base_clock_mhz: entry.base_clock_mhz,
            boost_clock_mhz: entry.boost_clock_mhz,
            l3_cache_mb: entry.l3_cache_mb,
            tdp_watts: entry.tdp_watts,
            performance_core_count: entry.performance_core_count,
            efficiency_core_count: entry.efficiency_core_count,
            reference_urls: entry.reference_urls.clone(),
            diagram_template: entry.diagram_template.clone(),
            reference_score: entry.cpu_reference_score,
        },
        catalog_source(),
    )
}

fn lookup_gpus(catalog: &HardwareCatalog, gpus: &[GpuInfo]) -> Capability<Vec<GpuCatalogMatch>> {
    if gpus.is_empty() {
        return Capability::unavailable("no gpu devices were detected");
    }

    let mut matches = Vec::new();
    for gpu in gpus {
        let normalized = normalize_model(&gpu.name);
        if let Some(entry) = catalog
            .gpus
            .iter()
            .find(|entry| entry_match(&normalized, &entry.match_patterns))
        {
            matches.push(GpuCatalogMatch {
                matched_model: entry.canonical_model.clone(),
                confidence: match_confidence(&normalized, &entry.match_patterns),
                codename: entry.codename.clone(),
                architecture_family: entry.architecture_family.clone(),
                architecture_summary: entry.architecture_summary.clone(),
                process_nm: entry.process_nm,
                shader_units: entry.shader_units,
                memory_type: entry.memory_type.clone(),
                memory_bus_bits: entry.memory_bus_bits,
                tdp_watts: entry.tdp_watts,
                reference_urls: entry.reference_urls.clone(),
                diagram_template: entry.diagram_template.clone(),
                reference_score: entry.gpu_reference_score,
            });
        } else if normalized.contains("apple") && normalized.contains("m") {
            if let Some(entry) = catalog
                .gpus
                .iter()
                .find(|entry| entry.canonical_model == "Apple M1 GPU")
            {
                matches.push(GpuCatalogMatch {
                    matched_model: entry.canonical_model.clone(),
                    confidence: MatchConfidence::Fuzzy,
                    codename: entry.codename.clone(),
                    architecture_family: entry.architecture_family.clone(),
                    architecture_summary: entry.architecture_summary.clone(),
                    process_nm: entry.process_nm,
                    shader_units: entry.shader_units,
                    memory_type: entry.memory_type.clone(),
                    memory_bus_bits: entry.memory_bus_bits,
                    tdp_watts: entry.tdp_watts,
                    reference_urls: entry.reference_urls.clone(),
                    diagram_template: entry.diagram_template.clone(),
                    reference_score: entry.gpu_reference_score,
                });
            }
        }
    }

    if matches.is_empty() {
        Capability::unavailable(format!(
            "detected gpu names are not present in catalog {}",
            catalog.version
        ))
    } else {
        Capability::available(matches, catalog_source())
    }
}

fn lookup_memory(
    catalog: &HardwareCatalog,
    modules: Option<&[MemoryModuleInfo]>,
) -> Capability<MemoryCatalogMatch> {
    let Some(modules) = modules else {
        return Capability::unavailable("memory module details are unavailable");
    };
    if modules.is_empty() {
        return Capability::unavailable("no memory modules were detected");
    }

    let mut search_keys = Vec::new();
    for module in modules {
        if let Some(mem_type) = module.mem_type.as_deref() {
            if let Some(speed) = module.speed_mts {
                search_keys.push(format!("{mem_type}-{speed}"));
                search_keys.push(format!("{mem_type} {speed}"));
            }
            search_keys.push(mem_type.to_owned());
        }
    }

    for key in &search_keys {
        let normalized = normalize_model(key);
        if let Some(entry) = catalog.memory_profiles.iter().find(|entry| {
            entry_match(&normalized, &entry.match_patterns)
        }) {
            return Capability::available(
                MemoryCatalogMatch {
                    matched_profile: format!(
                        "{}-{}",
                        entry.memory_type.as_deref().unwrap_or("memory"),
                        entry.speed_mts.unwrap_or(0)
                    ),
                    confidence: match_confidence(&normalized, &entry.match_patterns),
                    memory_type: entry.memory_type.clone(),
                    speed_mts: entry.speed_mts,
                    channels: entry.channels,
                    architecture_summary: entry.architecture_summary.clone(),
                    reference_urls: entry.reference_urls.clone(),
                    diagram_template: entry.diagram_template.clone(),
                },
                catalog_source(),
            );
        }
    }

    if modules.iter().any(|module| {
        module
            .mem_type
            .as_deref()
            .is_some_and(|value| value.contains("LPDDR"))
    }) {
        if let Some(entry) = catalog.memory_profiles.iter().find(|entry| {
            entry.memory_type.as_deref() == Some("LPDDR5")
        }) {
            return Capability::available(
                MemoryCatalogMatch {
                    matched_profile: "LPDDR5-unified".to_owned(),
                    confidence: MatchConfidence::Fuzzy,
                    memory_type: entry.memory_type.clone(),
                    speed_mts: entry.speed_mts,
                    channels: entry.channels,
                    architecture_summary: entry.architecture_summary.clone(),
                    reference_urls: entry.reference_urls.clone(),
                    diagram_template: entry.diagram_template.clone(),
                },
                catalog_source(),
            );
        }
    }

    Capability::unavailable(format!(
        "detected memory modules are not present in catalog {}",
        catalog.version
    ))
}

fn build_benchmark_reference(
    catalog: &HardwareCatalog,
    cpu: &Capability<CpuCatalogMatch>,
    gpu: &Capability<Vec<GpuCatalogMatch>>,
    cpu_workload: Option<&WorkloadResult>,
    gpu_workload: Option<&WorkloadResult>,
) -> Capability<BenchmarkReferenceMatch> {
    let cpu_reference_score = cpu.value.as_ref().and_then(|value| value.reference_score);
    let gpu_reference_score = gpu
        .value
        .as_ref()
        .and_then(|values| values.first())
        .and_then(|value| value.reference_score);

    if cpu_reference_score.is_none() && gpu_reference_score.is_none() {
        return Capability::unavailable("catalog benchmark reference scores are unavailable");
    }

    let cpu_local_rate = cpu_workload.map(|value| value.summary.median_rate_per_second);
    let gpu_local_rate = gpu_workload.map(|value| value.summary.median_rate_per_second);

    Capability::available(
        BenchmarkReferenceMatch {
            catalog_benchmark_version: catalog.benchmark_reference_version.clone(),
            score_source: catalog.score_source.clone(),
            cpu_reference_score,
            gpu_reference_score,
            cpu_local_rate_per_second: cpu_local_rate,
            gpu_local_rate_per_second: gpu_local_rate,
            cpu_vs_reference_basis_points: relative_basis_points(
                cpu_local_rate,
                cpu_reference_score,
            ),
            gpu_vs_reference_basis_points: relative_basis_points(
                gpu_local_rate,
                gpu_reference_score,
            ),
        },
        catalog_source(),
    )
}

fn relative_basis_points(local: Option<u64>, reference: Option<u64>) -> Option<i32> {
    let local = local?;
    let reference = reference?;
    if reference == 0 {
        return None;
    }
    let difference = i128::from(local) - i128::from(reference);
    i32::try_from(
        difference
            .saturating_mul(10_000)
            .checked_div(i128::from(reference))
            .unwrap_or(0)
            .clamp(i128::from(i32::MIN), i128::from(i32::MAX)),
    )
    .ok()
}

fn entry_match(normalized: &str, patterns: &[String]) -> bool {
    patterns
        .iter()
        .any(|pattern| matches_pattern(normalized, pattern))
}

fn match_confidence(normalized: &str, patterns: &[String]) -> MatchConfidence {
    if patterns.iter().any(|pattern| normalized == normalize_model(pattern)) {
        MatchConfidence::Exact
    } else if patterns
        .iter()
        .any(|pattern| normalized.contains(&normalize_model(pattern)))
    {
        MatchConfidence::Normalized
    } else {
        MatchConfidence::Fuzzy
    }
}

/// Renders a simplified architecture SVG for a catalog diagram template id.
#[must_use]
pub fn render_diagram(template: &str, title: &str) -> Option<String> {
    match template {
        "hybrid_cpu" => Some(hybrid_cpu_diagram(title)),
        "chiplet_cpu" => Some(chiplet_cpu_diagram(title)),
        "soc_unified" => Some(soc_unified_diagram(title)),
        "discrete_gpu" => Some(discrete_gpu_diagram(title)),
        "dual_channel_memory" => Some(dual_channel_memory_diagram(title)),
        "unified_memory" => Some(unified_memory_diagram(title)),
        _ => None,
    }
}

fn hybrid_cpu_diagram(title: &str) -> String {
    svg_wrapper(
        title,
        r##"<rect x="40" y="40" width="520" height="220" rx="12" fill="#eef3ff" stroke="#446"/>
<text x="60" y="70" font-size="14">P-cores</text>
<rect x="60" y="85" width="90" height="50" rx="8" fill="#9cf"/>
<rect x="165" y="85" width="90" height="50" rx="8" fill="#9cf"/>
<rect x="270" y="85" width="90" height="50" rx="8" fill="#9cf"/>
<rect x="375" y="85" width="90" height="50" rx="8" fill="#9cf"/>
<text x="60" y="170" font-size="14">E-cores</text>
<rect x="60" y="185" width="70" height="40" rx="8" fill="#bdf"/>
<rect x="145" y="185" width="70" height="40" rx="8" fill="#bdf"/>
<rect x="230" y="185" width="70" height="40" rx="8" fill="#bdf"/>
<rect x="315" y="185" width="70" height="40" rx="8" fill="#bdf"/>
<rect x="430" y="110" width="110" height="100" rx="8" fill="#ffd" stroke="#886"/>
<text x="445" y="165" font-size="13">Shared L3</text>"##,
    )
}

fn chiplet_cpu_diagram(title: &str) -> String {
    svg_wrapper(
        title,
        r##"<rect x="50" y="70" width="120" height="120" rx="10" fill="#eef3ff" stroke="#446"/>
<text x="70" y="135" font-size="13">CCD</text>
<rect x="210" y="70" width="120" height="120" rx="10" fill="#eef3ff" stroke="#446"/>
<text x="230" y="135" font-size="13">CCD</text>
<rect x="370" y="95" width="150" height="70" rx="10" fill="#ffd" stroke="#886"/>
<text x="395" y="135" font-size="13">I/O Die</text>"##,
    )
}

fn soc_unified_diagram(title: &str) -> String {
    svg_wrapper(
        title,
        r##"<rect x="40" y="50" width="520" height="220" rx="16" fill="#eef8ef" stroke="#484"/>
<text x="70" y="85" font-size="14">CPU clusters</text>
<rect x="70" y="100" width="140" height="60" rx="8" fill="#9cf"/>
<text x="250" y="85" font-size="14">GPU</text>
<rect x="250" y="100" width="140" height="60" rx="8" fill="#9f9"/>
<text x="430" y="85" font-size="14">NPU / Media</text>
<rect x="430" y="100" width="100" height="60" rx="8" fill="#fd9"/>
<rect x="70" y="190" width="460" height="50" rx="8" fill="#ffd" stroke="#886"/>
<text x="230" y="220" font-size="13">Unified memory pool</text>"##,
    )
}

fn discrete_gpu_diagram(title: &str) -> String {
    svg_wrapper(
        title,
        r##"<rect x="60" y="80" width="460" height="150" rx="14" fill="#eef3ff" stroke="#446"/>
<text x="90" y="115" font-size="14">Shader cores</text>
<rect x="90" y="130" width="180" height="70" rx="8" fill="#9cf"/>
<text x="300" y="115" font-size="14">RT / Tensor</text>
<rect x="300" y="130" width="90" height="70" rx="8" fill="#bdf"/>
<text x="410" y="115" font-size="14">VRAM</text>
<rect x="410" y="130" width="80" height="70" rx="8" fill="#ffd" stroke="#886"/>"##,
    )
}

fn dual_channel_memory_diagram(title: &str) -> String {
    svg_wrapper(
        title,
        r##"<rect x="80" y="100" width="160" height="90" rx="10" fill="#eef3ff" stroke="#446"/>
<text x="130" y="150" font-size="13">DIMM A</text>
<rect x="280" y="100" width="160" height="90" rx="10" fill="#eef3ff" stroke="#446"/>
<text x="330" y="150" font-size="13">DIMM B</text>
<line x1="160" y1="60" x2="360" y2="60" stroke="#446" stroke-width="3"/>
<text x="210" y="50" font-size="13">Memory controller</text>"##,
    )
}

fn unified_memory_diagram(title: &str) -> String {
    svg_wrapper(
        title,
        r##"<rect x="70" y="110" width="420" height="80" rx="12" fill="#ffd" stroke="#886"/>
<text x="220" y="155" font-size="13">Unified memory</text>
<line x1="120" y1="70" x2="120" y2="110" stroke="#484"/>
<line x1="260" y1="70" x2="260" y2="110" stroke="#484"/>
<line x1="400" y1="70" x2="400" y2="110" stroke="#484"/>
<text x="95" y="60" font-size="12">CPU</text>
<text x="235" y="60" font-size="12">GPU</text>
<text x="375" y="60" font-size="12">NPU</text>"##,
    )
}

fn svg_wrapper(title: &str, body: &str) -> String {
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 600 280\" role=\"img\" aria-label=\"{title}\"><title>{title}</title>{body}</svg>"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use mollow_core::GpuInfo;

    #[test]
    fn enrich_matches_intel_cpu_from_noisy_model_string() {
        let context = enrich(EnrichmentInput {
            cpu_model: Some("Intel(R) Core(TM) i7-12700K CPU @ 3.60GHz"),
            gpu_names: &[],
            memory_modules: None,
            cpu_workload: None,
            gpu_workload: None,
        })
        .expect("catalog should parse")
        .value
        .expect("cpu should match");

        assert_eq!(context.cpu.value.unwrap().matched_model, "Core i7-12700K");
    }

    #[test]
    fn enrich_builds_benchmark_reference_when_workloads_present() {
        let cpu_workload = WorkloadResult {
            workload_id: "cpu.fnv1a-stream".to_owned(),
            workload_version: 1,
            measurement: "bytes_per_second".to_owned(),
            warmup_iterations: 1,
            parameters: Vec::new(),
            samples: Vec::new(),
            summary: mollow_core::BenchmarkSummary {
                median_rate_per_second: 28_500,
                median_absolute_deviation: 0,
                minimum_rate_per_second: 28_500,
                maximum_rate_per_second: 28_500,
                variation_basis_points: 0,
            },
        };

        let reference = enrich(EnrichmentInput {
            cpu_model: Some("Core i7-12700K"),
            gpu_names: &[],
            memory_modules: None,
            cpu_workload: Some(&cpu_workload),
            gpu_workload: None,
        })
        .expect("catalog should parse")
        .value
        .expect("cpu should match")
        .benchmark_reference
        .value
        .expect("reference should be available");

        assert_eq!(reference.cpu_vs_reference_basis_points, Some(0));
    }

    #[test]
    fn render_diagram_returns_svg_for_known_template() {
        let svg = render_diagram("hybrid_cpu", "Hybrid CPU").expect("template should exist");
        assert!(svg.contains("<svg"));
    }

    #[test]
    fn enrich_matches_gpu_name() {
        let gpu = GpuInfo {
            name: "NVIDIA GeForce RTX 4090".to_owned(),
            vendor: Some("NVIDIA".to_owned()),
            driver_version: None,
            memory_bytes: None,
            apis: vec!["DRM".to_owned()],
        };
        let context = enrich(EnrichmentInput {
            cpu_model: None,
            gpu_names: &[gpu],
            memory_modules: None,
            cpu_workload: None,
            gpu_workload: None,
        })
        .expect("catalog should parse")
        .value
        .expect("gpu should match");

        assert_eq!(
            context.gpu.value.unwrap()[0].matched_model,
            "GeForce RTX 4090"
        );
    }
}
