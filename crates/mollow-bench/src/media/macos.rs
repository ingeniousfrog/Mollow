use mollow_core::BenchmarkProfile;

use crate::BenchmarkError;

pub(crate) fn run(
    profile: BenchmarkProfile,
) -> Result<mollow_core::WorkloadResult, BenchmarkError> {
    let configuration = crate::workloads::configuration(profile);
    let _ = configuration.media_warmup_iterations;
    let _ = profile;
    Err(BenchmarkError::new(
        "media",
        "VideoToolbox hardware encoder initialization is not yet stable on this host",
    ))
}
