#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

use mollow_core::BenchmarkProfile;

use crate::BenchmarkError;

pub(crate) fn run(
    profile: BenchmarkProfile,
) -> Result<mollow_core::WorkloadResult, BenchmarkError> {
    #[cfg(target_os = "macos")]
    {
        macos::run(profile)
    }
    #[cfg(target_os = "windows")]
    {
        windows::run(profile)
    }
    #[cfg(target_os = "linux")]
    {
        linux::run(profile)
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        let _ = profile;
        Err(BenchmarkError::new(
            "media",
            "platform media backend is unsupported",
        ))
    }
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
pub(crate) const H264_FIXTURE: &[u8] = include_bytes!("../../fixtures/minimal-baseline.h264");

#[cfg(any(target_os = "windows", target_os = "linux"))]
pub(crate) fn split_annex_b_nals(data: &[u8]) -> Vec<&[u8]> {
    let mut nals = Vec::new();
    let mut start = 0_usize;
    let mut index = 0_usize;
    while index + 3 < data.len() {
        let is_start_code = data[index] == 0
            && data[index + 1] == 0
            && (data[index + 2] == 1 || (data[index + 2] == 0 && data[index + 3] == 1));
        if is_start_code {
            if start < index {
                nals.push(&data[start..index]);
            }
            start = if data[index + 2] == 1 {
                index + 3
            } else {
                index + 4
            };
        }
        index += 1;
    }
    if start < data.len() {
        nals.push(&data[start..]);
    }
    nals
}
