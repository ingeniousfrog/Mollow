# Mollow architecture

Mollow separates machine facts from interpretation and presentation. Platform
code gathers facts through native APIs or thin FFI wrappers; it does not score
hardware or produce user-facing prose.

## Boundaries

```text
mollow-cli
    |
    +-- mollow-platform -- native OS adapters
    |        |
    |        +-- mollow-core -- versioned domain model
    |
    +-- mollow-bench ----- versioned workloads
    +-- mollow-compare --- comparability and change detection
    +-- mollow-archive --- local baseline archive indexing
    +-- mollow-report ---- JSON, terminal, Markdown, and HTML renderers
```

- `mollow-core` owns stable, serializable domain types. It must not depend on a
  specific operating system or report format.
- `mollow-platform` owns collection interfaces and native adapters. Collection
  failures become explicit capability states instead of missing or fabricated
  values.
- `mollow-bench` owns bounded, versioned workloads and robust sample summaries.
- `mollow-compare` owns comparability rules, regression thresholds, environment
  warnings, and field-level changes.
- `mollow-archive` owns local archive indexing and trend summaries.
- `mollow-report` renders the same snapshot into multiple representations.
- `mollow-cli` parses commands and coordinates the other crates.

## Capability semantics

Every major section reports one of these states:

- `available`: a value was collected and includes its source.
- `unsupported`: the machine or this Mollow build cannot provide the value.
- `permission_denied`: the platform exposes the value but access was denied.
- `unavailable`: the value is temporarily unavailable.
- `error`: collection failed unexpectedly and includes a diagnostic message.

An absent value is therefore never ambiguous.

## Schema evolution

Machine snapshots carry both `schema_version` and `mollow_version`. Compatible
additions may extend a schema, while breaking changes require a new schema file
and an explicit migration path. Schema v1 remains available for the Phase 1
shape; Phase 2 uses v2 because storage and runtime placeholders became typed
collections; Phase 6 uses v3 for typed GPU, media, power, and thermal sections.
Benchmark Schema v3 upgrades GPU and media from pending placeholders to typed
workload results. Report language and formatting never change the underlying
snapshot.

## Current platform coverage

The macOS adapter uses `sysctlbyname`, Mach host statistics, `getmntinfo`,
`system_profiler`, VideoToolbox, and `pmset` for operating-system, CPU, memory,
swap, mounted-volume, GPU, media, power, and thermal facts. Runtime discovery
executes a fixed allowlist of version commands directly without a shell or
user-controlled arguments.

The Linux adapter reads `/proc`, `/etc/os-release`, `uname`, `statvfs`, DRM
sysfs, VA-API, V4L2, power-supply sysfs, and thermal zones. Parsing of CPU,
memory, and mount records is isolated into fixture-tested pure functions.

The Windows adapter uses thin Win32/NT FFI for version, hostname, CPU features,
memory state, registry-backed CPU identity, mounted volumes, and power state.
GPU enumeration uses DXGI; media codec detection uses Media Foundation; thermal
state uses WMI when available. It passes cross-target type checking; live
Windows validation remains a release gate documented in
[release-verification.md](release-verification.md).
