use mollow_core::{
    BENCHMARK_SCHEMA_VERSION, BenchmarkContext, BenchmarkProfile, BenchmarkRun, BenchmarkSample,
    BenchmarkSummary, Capability, CpuInfo, DataSource, MachineSnapshot, MemoryInfo, SCHEMA_VERSION,
    SystemInfo, WorkloadResult,
};

pub(crate) fn fixture_snapshot(hostname: &str) -> MachineSnapshot {
    let source = DataSource {
        provider: "fixture".to_owned(),
        detail: None,
    };
    MachineSnapshot {
        schema_version: SCHEMA_VERSION.to_owned(),
        mollow_version: "0.1.0".to_owned(),
        captured_at_unix_ms: 1234,
        system: Capability::available(
            SystemInfo {
                os_name: "FixtureOS".to_owned(),
                os_version: Some("1.0".to_owned()),
                kernel_version: None,
                architecture: "fixture64".to_owned(),
                hostname: Some(hostname.to_owned()),
            },
            source.clone(),
        ),
        cpu: Capability::available(
            CpuInfo {
                model: Some("Fixture CPU".to_owned()),
                physical_cores: Some(4),
                logical_cores: 8,
                features: vec!["simd".to_owned()],
            },
            source.clone(),
        ),
        memory: Capability::available(
            MemoryInfo {
                total_bytes: 8 * 1024 * 1024 * 1024,
                available_bytes: Some(4 * 1024 * 1024 * 1024),
                swap: Capability::unsupported("fixture"),
            },
            source,
        ),
        storage: Capability::unsupported("fixture"),
        gpu: Capability::unsupported("fixture"),
        media: Capability::unsupported("fixture"),
        power: Capability::unsupported("fixture"),
        thermal: Capability::unsupported("fixture"),
        runtimes: Capability::unsupported("fixture"),
        warnings: Vec::new(),
    }
}

pub(crate) fn fixture_benchmark() -> BenchmarkRun {
    let workload = WorkloadResult {
        workload_id: "fixture".to_owned(),
        workload_version: 1,
        measurement: "bytes_per_second".to_owned(),
        warmup_iterations: 1,
        parameters: Vec::new(),
        samples: vec![BenchmarkSample {
            elapsed_ns: 1,
            work_units: 1,
            rate_per_second: 100,
        }],
        summary: BenchmarkSummary {
            median_rate_per_second: 100,
            median_absolute_deviation: 5,
            minimum_rate_per_second: 90,
            maximum_rate_per_second: 110,
            variation_basis_points: 500,
        },
    };
    let capability = || {
        Capability::available(
            workload.clone(),
            DataSource {
                provider: "fixture".to_owned(),
                detail: None,
            },
        )
    };
    BenchmarkRun {
        schema_version: BENCHMARK_SCHEMA_VERSION.to_owned(),
        mollow_version: "0.1.0".to_owned(),
        started_at_unix_ms: 1234,
        profile: BenchmarkProfile::Quick,
        context: BenchmarkContext {
            build_profile: "release".to_owned(),
            machine_snapshot: fixture_snapshot("fixture"),
        },
        cpu: capability(),
        memory: capability(),
        storage: capability(),
        gpu: Capability::unsupported("fixture"),
        media: Capability::unsupported("fixture"),
        warnings: vec!["fixture warning".to_owned()],
    }
}
