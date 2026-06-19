use std::fs::{File, OpenOptions};
use std::hint::black_box;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use mollow_core::{BenchmarkParameter, BenchmarkProfile, BenchmarkSample, WorkloadResult};

use crate::BenchmarkError;
use crate::statistics::summarize;

const NANOS_PER_SECOND: u128 = 1_000_000_000;
static TEMP_FILE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub(crate) fn run_cpu(profile: BenchmarkProfile) -> Result<WorkloadResult, BenchmarkError> {
    let configuration = configuration(profile);
    let input = deterministic_bytes(configuration.cpu_bytes, "cpu")?;

    for _ in 0..configuration.cpu_warmup_iterations {
        black_box(fnv1a(&input));
    }

    let samples = (0..configuration.sample_count)
        .map(|_| {
            timed_sample(
                u64::try_from(input.len())
                    .map_err(|error| BenchmarkError::new("cpu", error.to_string()))?,
                || {
                    black_box(fnv1a(&input));
                },
                "cpu",
            )
        })
        .collect::<Result<Vec<_>, _>>()?;

    workload_result(
        "cpu.fnv1a-stream",
        "bytes_per_second",
        configuration.cpu_warmup_iterations,
        vec![
            parameter("input_bytes", &input.len().to_string()),
            parameter("algorithm", "fnv1a-64"),
            parameter("threads", "1"),
        ],
        samples,
    )
}

pub(crate) fn run_memory(profile: BenchmarkProfile) -> Result<WorkloadResult, BenchmarkError> {
    let configuration = configuration(profile);
    let source = deterministic_bytes(configuration.memory_bytes, "memory")?;
    let mut destination = zeroed_bytes(source.len(), "memory")?;

    for _ in 0..configuration.memory_warmup_iterations {
        destination.copy_from_slice(&source);
        black_box(destination[0]);
    }

    let work_units = u64::try_from(source.len())
        .map_err(|error| BenchmarkError::new("memory", error.to_string()))?;
    let samples = (0..configuration.sample_count)
        .map(|_| {
            timed_sample(
                work_units,
                || {
                    destination.copy_from_slice(&source);
                    black_box(destination[source.len() - 1]);
                },
                "memory",
            )
        })
        .collect::<Result<Vec<_>, _>>()?;

    workload_result(
        "memory.sequential-copy",
        "bytes_per_second",
        configuration.memory_warmup_iterations,
        vec![
            parameter("buffer_bytes", &source.len().to_string()),
            parameter("operation", "copy_from_slice"),
            parameter("threads", "1"),
        ],
        samples,
    )
}

pub(crate) fn run_storage(profile: BenchmarkProfile) -> Result<WorkloadResult, BenchmarkError> {
    let configuration = configuration(profile);
    let input = deterministic_bytes(configuration.storage_bytes, "storage")?;
    let mut temporary_file = TemporaryFile::create()
        .map_err(|error| BenchmarkError::new("storage", error.to_string()))?;
    let mut read_buffer = zeroed_bytes(input.len(), "storage")?;

    let result = (|| {
        for _ in 0..configuration.storage_warmup_iterations {
            write_read_cycle(temporary_file.file_mut()?, &input, &mut read_buffer)
                .map_err(|error| BenchmarkError::new("storage", error.to_string()))?;
            verify_readback(&input, &read_buffer)?;
        }

        let bytes_per_cycle = u64::try_from(input.len())
            .map_err(|error| BenchmarkError::new("storage", error.to_string()))?
            .checked_mul(2)
            .ok_or_else(|| BenchmarkError::new("storage", "work unit count overflowed"))?;
        let samples = (0..configuration.sample_count)
            .map(|_| {
                let started = Instant::now();
                write_read_cycle(temporary_file.file_mut()?, &input, &mut read_buffer)
                    .map_err(|error| BenchmarkError::new("storage", error.to_string()))?;
                let elapsed_ns = started.elapsed().as_nanos();
                verify_readback(&input, &read_buffer)?;
                sample_from_elapsed(bytes_per_cycle, elapsed_ns, "storage")
            })
            .collect::<Result<Vec<_>, _>>()?;

        workload_result(
            "storage.sequential-write-read",
            "bytes_per_second",
            configuration.storage_warmup_iterations,
            vec![
                parameter("file_bytes", &input.len().to_string()),
                parameter("sync", "sync_all"),
                parameter("read_after_write", "true"),
                parameter("threads", "1"),
            ],
            samples,
        )
    })();

    let cleanup = temporary_file
        .cleanup()
        .map_err(|error| BenchmarkError::new("storage", format!("cleanup failed: {error}")));
    match (result, cleanup) {
        (Ok(workload), Ok(())) => Ok(workload),
        (Err(error), Ok(())) => Err(error),
        (Ok(_), Err(cleanup_error)) => Err(cleanup_error),
        (Err(error), Err(cleanup_error)) => Err(BenchmarkError::new(
            "storage",
            format!("{error}; {cleanup_error}"),
        )),
    }
}

pub(crate) fn run_gpu(profile: BenchmarkProfile) -> Result<WorkloadResult, BenchmarkError> {
    let configuration = configuration(profile);
    let size = match profile {
        BenchmarkProfile::Quick => 128,
        BenchmarkProfile::Standard => 256,
    };
    let left = deterministic_matrix(size, 1, "gpu-left")?;
    let right = deterministic_matrix(size, 2, "gpu-right")?;
    let mut output = vec![0.0_f32; size * size];

    for _ in 0..configuration.gpu_warmup_iterations {
        matrix_multiply(&left, &right, &mut output, size);
        black_box(output[0]);
    }

    let work_units = u64::try_from(size)
        .map_err(|error| BenchmarkError::new("gpu", error.to_string()))?
        .checked_pow(3)
        .ok_or_else(|| BenchmarkError::new("gpu", "work unit count overflowed"))?;
    let samples = (0..configuration.sample_count)
        .map(|_| {
            timed_sample(
                work_units,
                || {
                    matrix_multiply(&left, &right, &mut output, size);
                    black_box(output[size * size - 1]);
                },
                "gpu",
            )
        })
        .collect::<Result<Vec<_>, _>>()?;

    workload_result(
        "gpu.matrix-multiply",
        "flops_per_second",
        configuration.gpu_warmup_iterations,
        vec![
            parameter("matrix_size", &size.to_string()),
            parameter("element_type", "f32"),
            parameter("backend", "host-simd"),
        ],
        samples,
    )
}

pub(crate) fn run_media(profile: BenchmarkProfile) -> Result<WorkloadResult, BenchmarkError> {
    let configuration = configuration(profile);
    let frame_bytes = match profile {
        BenchmarkProfile::Quick => 1280 * 720 * 3 / 2,
        BenchmarkProfile::Standard => 1920 * 1080 * 3 / 2,
    };
    let frame = deterministic_bytes(frame_bytes, "media")?;
    let mut scratch = zeroed_bytes(frame.len(), "media")?;

    for _ in 0..configuration.media_warmup_iterations {
        process_frame(&frame, &mut scratch);
        black_box(scratch[0]);
    }

    let work_units = u64::try_from(frame.len())
        .map_err(|error| BenchmarkError::new("media", error.to_string()))?;
    let samples = (0..configuration.sample_count)
        .map(|_| {
            timed_sample(
                work_units,
                || {
                    process_frame(&frame, &mut scratch);
                    black_box(scratch[frame.len() - 1]);
                },
                "media",
            )
        })
        .collect::<Result<Vec<_>, _>>()?;

    workload_result(
        "media.frame-bytes-process",
        "bytes_per_second",
        configuration.media_warmup_iterations,
        vec![
            parameter("frame_bytes", &frame.len().to_string()),
            parameter("format", "nv12-like"),
            parameter("operation", "deterministic-transform"),
        ],
        samples,
    )
}

fn deterministic_matrix(
    size: usize,
    seed: u32,
    workload: &'static str,
) -> Result<Vec<f32>, BenchmarkError> {
    let mut values = Vec::new();
    values.try_reserve_exact(size * size).map_err(|error| {
        BenchmarkError::new(workload, format!("matrix allocation failed: {error}"))
    })?;
    let mut state = seed;
    for _ in 0..size * size {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        values.push(f32::from((state & 0xff) as u8) / 255.0);
    }
    Ok(values)
}

fn matrix_multiply(left: &[f32], right: &[f32], output: &mut [f32], size: usize) {
    for row in 0..size {
        for col in 0..size {
            let mut sum = 0.0_f32;
            for index in 0..size {
                sum += left[row * size + index] * right[index * size + col];
            }
            output[row * size + col] = sum;
        }
    }
}

fn process_frame(frame: &[u8], scratch: &mut [u8]) {
    for (index, byte) in frame.iter().enumerate() {
        scratch[index] = byte.rotate_left(u32::try_from(index % 8).unwrap_or(0)) ^ 0x5a;
    }
}

fn workload_result(
    workload_id: &str,
    measurement: &str,
    warmup_iterations: u32,
    parameters: Vec<BenchmarkParameter>,
    samples: Vec<BenchmarkSample>,
) -> Result<WorkloadResult, BenchmarkError> {
    let summary = summarize(&samples)?;
    Ok(WorkloadResult {
        workload_id: workload_id.to_owned(),
        workload_version: 1,
        measurement: measurement.to_owned(),
        warmup_iterations,
        parameters,
        samples,
        summary,
    })
}

fn timed_sample(
    work_units: u64,
    operation: impl FnOnce(),
    workload: &'static str,
) -> Result<BenchmarkSample, BenchmarkError> {
    let started = Instant::now();
    operation();
    sample_from_elapsed(work_units, started.elapsed().as_nanos(), workload)
}

fn sample_from_elapsed(
    work_units: u64,
    elapsed_ns: u128,
    workload: &'static str,
) -> Result<BenchmarkSample, BenchmarkError> {
    if elapsed_ns == 0 {
        return Err(BenchmarkError::new(
            workload,
            "timer returned zero duration",
        ));
    }
    let rate = u128::from(work_units)
        .saturating_mul(NANOS_PER_SECOND)
        .checked_div(elapsed_ns)
        .ok_or_else(|| BenchmarkError::new(workload, "could not calculate sample rate"))?;

    Ok(BenchmarkSample {
        elapsed_ns: u64::try_from(elapsed_ns)
            .map_err(|error| BenchmarkError::new(workload, error.to_string()))?,
        work_units,
        rate_per_second: u64::try_from(rate)
            .map_err(|error| BenchmarkError::new(workload, error.to_string()))?,
    })
}

fn fnv1a(input: &[u8]) -> u64 {
    input.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

fn deterministic_bytes(length: usize, workload: &'static str) -> Result<Vec<u8>, BenchmarkError> {
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(length).map_err(|error| {
        BenchmarkError::new(workload, format!("input allocation failed: {error}"))
    })?;
    let mut state = 0x9e37_79b9_u32;
    for _ in 0..length {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        bytes.push((state & 0xff) as u8);
    }
    Ok(bytes)
}

fn zeroed_bytes(length: usize, workload: &'static str) -> Result<Vec<u8>, BenchmarkError> {
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(length).map_err(|error| {
        BenchmarkError::new(workload, format!("buffer allocation failed: {error}"))
    })?;
    bytes.resize(length, 0);
    Ok(bytes)
}

fn write_read_cycle(file: &mut File, input: &[u8], read_buffer: &mut [u8]) -> std::io::Result<()> {
    file.set_len(0)?;
    file.seek(SeekFrom::Start(0))?;
    file.write_all(input)?;
    file.sync_all()?;
    file.seek(SeekFrom::Start(0))?;

    file.read_exact(read_buffer)?;
    black_box(read_buffer);
    Ok(())
}

fn verify_readback(input: &[u8], read_buffer: &[u8]) -> Result<(), BenchmarkError> {
    if read_buffer == input {
        Ok(())
    } else {
        Err(BenchmarkError::new(
            "storage",
            "storage readback did not match the written data",
        ))
    }
}

fn parameter(name: &str, value: &str) -> BenchmarkParameter {
    BenchmarkParameter {
        name: name.to_owned(),
        value: value.to_owned(),
    }
}

#[derive(Clone, Copy)]
struct Configuration {
    sample_count: usize,
    cpu_warmup_iterations: u32,
    memory_warmup_iterations: u32,
    storage_warmup_iterations: u32,
    gpu_warmup_iterations: u32,
    media_warmup_iterations: u32,
    cpu_bytes: usize,
    memory_bytes: usize,
    storage_bytes: usize,
}

fn configuration(profile: BenchmarkProfile) -> Configuration {
    match profile {
        BenchmarkProfile::Quick => Configuration {
            sample_count: 3,
            cpu_warmup_iterations: 8,
            memory_warmup_iterations: 2,
            storage_warmup_iterations: 1,
            gpu_warmup_iterations: 1,
            media_warmup_iterations: 1,
            cpu_bytes: 4 * 1024 * 1024,
            memory_bytes: 16 * 1024 * 1024,
            storage_bytes: 8 * 1024 * 1024,
        },
        BenchmarkProfile::Standard => Configuration {
            sample_count: 5,
            cpu_warmup_iterations: 3,
            memory_warmup_iterations: 2,
            storage_warmup_iterations: 1,
            gpu_warmup_iterations: 2,
            media_warmup_iterations: 2,
            cpu_bytes: 32 * 1024 * 1024,
            memory_bytes: 64 * 1024 * 1024,
            storage_bytes: 64 * 1024 * 1024,
        },
    }
}

struct TemporaryFile {
    path: Option<PathBuf>,
    file: Option<File>,
}

impl TemporaryFile {
    fn create() -> std::io::Result<Self> {
        let sequence = TEMP_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "mollow-storage-{}-{sequence}.tmp",
            std::process::id()
        ));
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&path)?;

        Ok(Self {
            path: Some(path),
            file: Some(file),
        })
    }

    fn file_mut(&mut self) -> Result<&mut File, BenchmarkError> {
        self.file
            .as_mut()
            .ok_or_else(|| BenchmarkError::new("storage", "temporary file is already closed"))
    }

    fn cleanup(mut self) -> std::io::Result<()> {
        drop(self.file.take());
        if let Some(path) = self.path.take() {
            std::fs::remove_file(path)?;
        }
        Ok(())
    }
}

impl Drop for TemporaryFile {
    fn drop(&mut self) {
        drop(self.file.take());
        if let Some(path) = self.path.take() {
            let _ = std::fs::remove_file(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quick_cpu_workload_returns_repeated_nonzero_samples() {
        let result = run_cpu(BenchmarkProfile::Quick).expect("CPU workload should run");

        assert_eq!(result.workload_id, "cpu.fnv1a-stream");
        assert_eq!(result.samples.len(), 3);
        assert!(result.samples.iter().all(|sample| sample.elapsed_ns > 0));
    }

    #[test]
    fn quick_memory_workload_reports_copied_bytes() {
        let result = run_memory(BenchmarkProfile::Quick).expect("memory workload should run");

        assert_eq!(result.workload_id, "memory.sequential-copy");
        assert!(result.samples.iter().all(|sample| sample.work_units > 0));
    }

    #[test]
    fn quick_storage_workload_cleans_up_its_temporary_file() {
        let before = temporary_benchmark_files();

        run_storage(BenchmarkProfile::Quick).expect("storage workload should run");

        assert_eq!(temporary_benchmark_files(), before);
    }

    fn temporary_benchmark_files() -> Vec<PathBuf> {
        let mut files = std::fs::read_dir(std::env::temp_dir())
            .expect("temporary directory should be readable")
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with("mollow-storage-"))
            })
            .collect::<Vec<_>>();
        files.sort();
        files
    }
}
