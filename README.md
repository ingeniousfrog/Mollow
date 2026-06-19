# Mollow

English | [简体中文](README-CN.md)

Mollow is a cross-platform machine inspection and performance-baseline CLI.
It answers three practical questions:

1. **Capability** — What hardware and runtime stack is available on this machine?
2. **State** — What is the current operating environment (power, thermal, memory pressure)?
3. **Change** — How does a new run differ from a saved baseline or another machine?

Mollow combines versioned machine snapshots with small, reproducible workloads.
It is designed for regression checks and environment audits, not as a replacement
for specialist benchmark suites, hardware monitors, or tuning tools.

## Quick start

Requirements: Rust **1.85+** (see `rust-version` in `Cargo.toml`).

```bash
cargo build --workspace --release

# Inspect current machine state
cargo run --release -p mollow -- inspect --format terminal --lang zh-CN

# Run a quick benchmark (use --release for comparable baselines)
cargo run --release -p mollow -- bench --profile quick --format json

# Capture, compare, and report
cargo run --release -p mollow -- capture --output baseline.json
cargo run -p mollow -- compare baseline.json candidate.json --format markdown
cargo run -p mollow -- report baseline.json --format html --output report.html

# Local archive and trends
cargo run -p mollow -- archive add baseline.json --dir ~/.mollow/archive
cargo run -p mollow -- archive list --dir ~/.mollow/archive
cargo run -p mollow -- archive trend --dir ~/.mollow/archive --workload cpu
```

## Install with Homebrew (macOS)

Mollow is a **CLI binary**, so it belongs in a Homebrew **Formula** (not a Cask like GUI apps).
The same [ingeniousfrog/homebrew-tap](https://github.com/ingeniousfrog/homebrew-tap) used for CacheBar can host it:

```bash
brew tap ingeniousfrog/tap
brew install mollow
```

This requires a published GitHub Release tarball per architecture. See [docs/homebrew.md](docs/homebrew.md) for the maintainer workflow and the `packaging/homebrew/mollow.rb` template.

## Commands

| Command | Purpose |
| --- | --- |
| `inspect` | Collect a versioned machine snapshot (Schema v3) |
| `bench` | Run CPU, memory, storage, GPU, and media workloads (Benchmark Schema v3) |
| `capture` | Snapshot + benchmark in one artifact |
| `compare` | Diff two captures or benchmark runs with comparability rules |
| `report` | Render JSON, terminal, Markdown, or semantic HTML (en / zh-CN) |
| `archive` | Add, list, and trend local baseline files |

## Workloads (v2)

| Domain | Workload ID | Backend |
| --- | --- | --- |
| CPU | `cpu.fnv1a-stream` | Host FNV-1a over deterministic input |
| Memory | `memory.sequential-copy` | Host sequential `copy_from_slice` |
| Storage | `storage.sequential-write-read` | Temp file write, `sync_all`, verified read |
| GPU | `gpu.wgpu-matrix-multiply` | **wgpu** compute shader (Metal / Vulkan / DX12) |
| Media (macOS) | `media.videotoolbox-h264-encode` | **VideoToolbox** hardware H.264 encode |
| Media (Windows) | `media.media-foundation-h264-decode` | **Media Foundation** hardware H.264 decode |
| Media (Linux) | `media.vaapi-h264-decode` | **VA-API** hardware H.264 decode |

Profiles: `quick` (3 samples, smaller inputs) and `standard` (5 samples, larger inputs).
See [benchmark methodology](docs/benchmarks.md) for warmup counts, statistics, and storage safety.

## Platform probes

| Platform | System / CPU / memory / storage | GPU | Media codecs | Power | Thermal |
| --- | --- | --- | --- | --- | --- |
| macOS | Native APIs | `system_profiler` | VideoToolbox | IOKit battery | SMC / thermal |
| Linux | `/proc`, sysfs | DRM | VA-API / V4L2 | power-supply sysfs | thermal zones |
| Windows | Win32 / NT | DXGI | Media Foundation | Win32 power | WMI |

Unsupported probes are recorded explicitly (`unsupported` / `error`) rather than inferred from device names.

## Comparison rules

A benchmark comparison is **comparable** only when:

- both files share the same benchmark schema, profile, and release build;
- workload IDs, versions, measurements, and parameters match;
- **environment is stable** — see strict rules below.

Median rates are classified with a default threshold of **500 basis points (5%)**:
regression (≤ −5%), improvement (≥ +5%), or stable.

### Strict environment mode

The following conditions mark a run as **not comparable** (in addition to schema/profile mismatches):

- power source differs between baseline and candidate;
- either capture was taken on **battery**;
- **low power mode** is enabled on either side;
- either side reports thermal state **warning** or **critical**.

`environment_warnings` are still emitted for terminal, Markdown, and HTML output.
Machine field changes (OS, CPU, memory, GPU, runtimes) are listed separately and never silently alter percentage deltas.

Details: [baseline comparison](docs/comparison.md).

## Workspace layout

| Crate / path | Role |
| --- | --- |
| `crates/mollow-core` | Versioned domain model and capability semantics |
| `crates/mollow-platform` | Native collection adapters (macOS / Linux / Windows) |
| `crates/mollow-bench` | Versioned workloads including wgpu GPU and platform media decode |
| `crates/mollow-compare` | Comparability checks, strict environment rules, field-level diffs |
| `crates/mollow-report` | Bilingual JSON, terminal, Markdown, and HTML renderers |
| `crates/mollow-archive` | Local baseline archive indexing and trends |
| `crates/mollow-cli` | CLI and application coordination |
| `schemas/` | JSON Schema export contracts (snapshot v3, benchmark v3, comparison v2) |
| `docs/` | Architecture, benchmarks, comparison, release verification |

## Quality checks

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test --workspace --release
```

Before publishing or archiving performance baselines, follow the
[release verification checklist](docs/release-verification.md).

## Documentation

- [Architecture](docs/architecture.md) — crate boundaries and schema evolution
- [Benchmarks](docs/benchmarks.md) — profiles, statistics, workload parameters
- [Comparison](docs/comparison.md) — comparability and strict environment rules
- [Release verification](docs/release-verification.md) — pre-release manual checks

## License

Apache License 2.0 — see [`LICENSE`](LICENSE).
