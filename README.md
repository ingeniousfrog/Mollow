# Mollow

Mollow is a cross-platform machine inspection and performance-baseline CLI.
It is designed to answer three questions:

1. What can this machine do?
2. What state is it in now?
3. What changed compared with an earlier baseline or another machine?

Mollow collects hardware, operating-system, and runtime facts, then pairs them
with lightweight and reproducible real-world workloads. It does not try to
replace full benchmark suites, hardware monitors, or tuning tools.

> Mollow 是一个跨平台的机器环境探测与性能基线 CLI。它关注机器具备什么能力、
> 当前处于什么状态，以及相对历史记录或其他机器发生了什么变化。

## Current milestone

The first vertical slice provides a versioned machine snapshot with:

- macOS system, CPU, and installed-memory collection through thin native FFI;
- explicit capability states and collection provenance;
- stable, pretty-printed JSON output;
- placeholders that clearly mark planned storage, GPU, media, power, thermal,
  and runtime collectors as unsupported.

Linux and Windows currently use a conservative portable adapter. Full native
collectors, benchmarks, comparison, and additional report formats are planned
next.

## Build and run

From the repository root:

```bash
cargo build --workspace
cargo run -p mollow -- inspect --format json
```

Run the quality checks:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Workspace

- `crates/mollow-core`: versioned domain model and capability semantics
- `crates/mollow-platform`: collection contracts and native adapters
- `crates/mollow-report`: JSON and future human-readable renderers
- `crates/mollow-cli`: command-line interface and application coordination
- `schemas`: versioned export contracts
- `docs`: architecture and decision records

See [the architecture guide](docs/architecture.md) for design boundaries and
schema evolution rules.
