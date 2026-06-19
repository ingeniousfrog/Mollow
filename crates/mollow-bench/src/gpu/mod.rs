use mollow_core::BenchmarkProfile;

use crate::statistics::summarize;
use mollow_core::BenchmarkSample;

use crate::BenchmarkError;
use crate::workloads::{configuration, parameter, sample_from_elapsed};

mod wgpu_backend;

const WORKLOAD_ID: &str = "gpu.wgpu-matrix-multiply";
const WORKLOAD_VERSION: u32 = 2;

pub(crate) fn run(
    profile: BenchmarkProfile,
) -> Result<mollow_core::WorkloadResult, BenchmarkError> {
    let configuration = configuration(profile);
    let size = match profile {
        BenchmarkProfile::Quick => 256,
        BenchmarkProfile::Standard => 512,
    };
    let backend = wgpu_backend::WgpuMatrixBackend::initialize(size)?;
    let adapter_name = backend.adapter_name.clone();
    let api = backend.api.clone();

    for _ in 0..configuration.gpu_warmup_iterations {
        backend.dispatch();
    }

    let work_units = u64::try_from(size)
        .map_err(|error| BenchmarkError::new("gpu", error.to_string()))?
        .checked_pow(3)
        .ok_or_else(|| BenchmarkError::new("gpu", "work unit count overflowed"))?;

    let samples = (0..configuration.sample_count)
        .map(|_| {
            let started = std::time::Instant::now();
            backend.dispatch();
            sample_from_elapsed(work_units, started.elapsed().as_nanos(), "gpu")
        })
        .collect::<Result<Vec<BenchmarkSample>, _>>()?;

    let summary = summarize(&samples)?;
    Ok(mollow_core::WorkloadResult {
        workload_id: WORKLOAD_ID.to_owned(),
        workload_version: WORKLOAD_VERSION,
        measurement: "flops_per_second".to_owned(),
        warmup_iterations: configuration.gpu_warmup_iterations,
        parameters: vec![
            parameter("matrix_size", &size.to_string()),
            parameter("element_type", "f32"),
            parameter("backend", "wgpu"),
            parameter("api", &api),
            parameter("adapter", &adapter_name),
        ],
        samples,
        summary,
    })
}
