# Mollow

English | [简体中文](README-CN.md)

Mollow is a cross-platform machine inspection and performance-baseline CLI.
It is designed to answer three questions:

1. What can this machine do?
2. What state is it in now?
3. What changed compared with an earlier baseline or another machine?

Mollow collects hardware, operating-system, and runtime facts, then pairs them
with lightweight and reproducible real-world workloads. It does not try to
replace full benchmark suites, hardware monitors, or tuning tools.

## Current milestone

The first vertical slice provides a versioned machine snapshot with:

- macOS system, CPU topology/features, live memory, mounted-volume, and
  installed-runtime collection through native APIs and thin FFI;
- explicit capability states and collection provenance;
- stable, pretty-printed JSON output;
- field-level status for restricted observations such as swap usage;
- placeholders that clearly mark planned GPU, media, power, and thermal
  collectors as unsupported.

Linux has native `/proc`, `uname`, and `statvfs` collectors with fixture-tested
parsers. Windows uses thin Win32/NT FFI for system, CPU, memory, and volume
facts. Both adapters pass cross-target Rust checks; live Windows verification
remains pending. Benchmarks, comparison, and additional report formats follow
in later phases.

## Build and run

From the repository root:

```bash
cargo build --workspace
cargo run -p mollow -- inspect --format terminal --lang zh-CN
cargo run --release -p mollow -- bench --profile quick --format json
cargo run --release -p mollow -- capture --output baseline.json
cargo run -p mollow -- compare baseline.json candidate.json --format markdown
cargo run -p mollow -- report baseline.json --format html --output report.html
```

Run the quality checks:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Workspace

- `crates/mollow-core`: versioned domain model and capability semantics
- `crates/mollow-bench`: versioned CPU, memory, and storage workloads
- `crates/mollow-compare`: comparability checks and field-level baseline changes
- `crates/mollow-platform`: collection contracts and native adapters
- `crates/mollow-report`: bilingual JSON, terminal, Markdown, and HTML renderers
- `crates/mollow-cli`: command-line interface and application coordination
- `schemas`: versioned export contracts
- `docs`: architecture and decision records

See [the architecture guide](docs/architecture.md) for design boundaries and
schema evolution rules. See [the benchmark methodology](docs/benchmarks.md)
before comparing or archiving performance results.

## Current capabilities

- macOS: native system/CPU/memory/storage probes, GPU via `system_profiler`,
  VideoToolbox hardware decode detection, battery/power state, and thermal
  status when the operating system exposes it.
- Linux: `/proc`, sysfs, DRM, power-supply, and thermal-zone probes.
- Windows: Win32/NT system, CPU, memory, storage, and power probes; GPU, media,
  and thermal probes remain explicitly unsupported.
- Reports: JSON, terminal, Markdown, and self-contained HTML in English or
  Chinese.
- Baselines: capture, compare, machine/runtime change detection, and explicit
  non-comparability reasons.
