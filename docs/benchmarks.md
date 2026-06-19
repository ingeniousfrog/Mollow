# Benchmark methodology

Mollow benchmarks are small, versioned workloads intended for repeatable
baselines. They are not synthetic peak-performance scores and they do not
replace specialist benchmark suites.

## Profiles

`quick` is designed for local diagnosis and frequent regression checks. It uses
three measured samples, with workload-specific warmups:

- CPU: hash a deterministic 4 MiB byte stream with FNV-1a after eight warmups.
- Memory: copy a deterministic 16 MiB buffer sequentially after two warmups.
- Storage: write, `sync_all`, and read back an 8 MiB temporary file after one
  warmup.

`standard` increases those inputs to 32 MiB, 64 MiB, and 64 MiB, with two
or three warmups depending on the workload, and five measured samples.

Every workload is currently single-threaded. Parameters, workload version,
elapsed nanoseconds, work units, and rates are stored with every run.
The run also embeds a machine snapshot captured immediately before measurement,
so system version, available memory, storage state, and explicit unsupported
power or thermal observations remain attached to the result.

## Statistics

Mollow reports the median rate, minimum, maximum, median absolute deviation
(MAD), and `MAD / median` in basis points. Median and MAD are preferred over a
mean because short local runs can contain scheduler and filesystem outliers.
Runs with more than 500 basis points (5%) variation receive an explicit noise
warning.

No CPU, memory, and storage values are merged into a single score.

## Comparability

For useful comparisons:

- build and run Mollow with `--release`;
- use the same Mollow and workload versions;
- use the same profile and workload parameters;
- keep power mode, thermal state, free storage, and background load comparable;
- inspect raw samples and variation, not only the median.

Debug builds remain runnable for development but emit a warning and should not
be saved as performance baselines.

## Storage safety

The storage workload creates a uniquely named file in the operating system's
temporary directory with `create_new`. File size is bounded by the selected
profile. The file is removed through an RAII guard on success or error, and
readback is verified before the sample is accepted.

The reported storage rate combines synchronized sequential write bytes and
sequential readback bytes. It may still be influenced by filesystem caching,
encryption, free space, and the temporary directory's backing volume.

## Deferred workloads

GPU and media benchmarks now run deterministic host-side workloads:

- GPU: `gpu.matrix-multiply`
- Media: `media.frame-bytes-process`

Platform-native hardware codec and GPU compute backends may extend these
workloads in later revisions. Mollow does not infer those results from device
names alone.
