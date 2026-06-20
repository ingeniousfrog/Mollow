# Mollow

English | [简体中文](README-CN.md)

**Mollow** is a cross-platform CLI for machine inspection and performance baselines.
It collects versioned hardware and runtime facts, runs small reproducible workloads,
and compares results across time or machines—with explicit rules for when a diff is
statistically meaningful.

Mollow is built for **environment audits**, **regression checks**, and **baseline
tracking**. It is not a replacement for full benchmark suites, continuous profilers,
or system tuning tools.

---

## What Mollow does

| Area | Capability |
| --- | --- |
| **Machine snapshot** (`inspect`) | OS, CPU, memory, storage volumes, GPU, media codecs, power, thermal state, installed runtimes |
| **Live monitoring** (`watch`) | Refresh memory, power, and thermal readings at a fixed interval |
| **Benchmarks** (`bench` / `capture`) | Versioned CPU, memory, storage, GPU (wgpu), and platform media workloads with median + MAD statistics |
| **Comparison** (`compare`) | Schema/profile/workload validation, strict environment checks, field-level machine diffs, regression classification |
| **Reporting** (`report`) | Same artifact rendered as terminal, JSON, Markdown, or semantic HTML (English / 简体中文) |
| **Archive** (`archive`) | Local baseline index and per-workload trend lines |

Every probe uses a **capability model**: values are `available`, `unsupported`, `error`,
or `permission_denied`—never inferred from device names alone.

### Benchmark workloads (v2)

| Domain | Workload ID | Backend |
| --- | --- | --- |
| CPU | `cpu.fnv1a-stream` | Host FNV-1a hash over deterministic input |
| Memory | `memory.sequential-copy` | Host sequential `copy_from_slice` |
| Storage | `storage.sequential-write-read` | Temp-file write, `sync_all`, verified read |
| GPU | `gpu.wgpu-matrix-multiply` | wgpu compute shader (Metal / Vulkan / DX12) |
| Media (macOS) | `media.videotoolbox-h264-encode` | VideoToolbox hardware H.264 encode |
| Media (Windows) | `media.media-foundation-h264-decode` | Media Foundation hardware H.264 decode |
| Media (Linux) | `media.vaapi-h264-decode` | VA-API hardware H.264 decode |

### Snapshot schema (v3) — fields collected by `inspect`

| Component | Examples |
| --- | --- |
| `system` | OS name/version, kernel, architecture, hostname |
| `cpu` | Model, physical/logical cores, ISA features |
| `memory` | Total/available RAM, swap usage |
| `storage` | Mount points, volume size, filesystem type |
| `gpu` | Device name, vendor, APIs |
| `media` | Backend, hardware decode/encode codec lists |
| `power` | AC/battery, charge %, low-power mode |
| `thermal` | State, temperature, sensor |
| `runtimes` | rustc, cargo, git, node, python (when present) |

---

## How it fits together

```mermaid
flowchart LR
  subgraph collect [Collect]
    I[inspect]
    B[bench]
    C[capture]
  end
  subgraph artifacts [Artifacts]
    S[(Snapshot JSON)]
    R[(Benchmark JSON)]
  end
  subgraph analyze [Analyze]
    P[compare]
    T[archive trend]
    H[report HTML]
  end
  I --> S
  B --> R
  C --> R
  S --> P
  R --> P
  R --> T
  S --> H
  R --> H
  P --> H
```

**Typical baseline workflow**

```mermaid
sequenceDiagram
  participant U as User
  participant M as mollow
  U->>M: capture --profile quick -o baseline.json
  Note over M: snapshot + benchmark in one file
  U->>M: capture --profile quick -o candidate.json
  U->>M: compare baseline.json candidate.json --format markdown
  M-->>U: comparable? workload deltas, machine changes
  U->>M: archive add --dir ~/.mollow/archive baseline.json
  U->>M: archive trend --dir ~/.mollow/archive --workload cpu
```

---

## Installation

Current release: **[v0.1.1](https://github.com/ingeniousfrog/Mollow/releases/tag/v0.1.1)**.
Prebuilt binaries are published for macOS (Apple Silicon and Intel), Linux x86_64, and Windows x86_64.

### Quick pick by platform

| Platform | Recommended | One-liner |
| --- | --- | --- |
| **macOS** | Homebrew | `brew tap ingeniousfrog/tap && brew install mollow` |
| **Linux** (generic) | Install script | See [Linux → Option A](#linux) below |
| **Ubuntu / Debian** | Ubuntu script | See [Linux → Option B](#linux) below |
| **Windows** | PowerShell script | See [Windows → Option A](#windows) below |
| **Any** (developers) | Build from source | `cargo build --release -p mollow` |

> **Not available:** `apt install mollow` — Mollow is not in Debian/Ubuntu official repositories.
> Use the Ubuntu install script, Homebrew on Linux, or a GitHub Release binary instead.

After installation, verify:

```bash
mollow --version
mollow inspect --format terminal --lang zh-CN
```

### Upgrade

Homebrew **does not auto-upgrade** when a new release ships. If you installed an older
version, refresh the tap before upgrading:

```bash
brew update
brew upgrade mollow
mollow --version
```

Check what Homebrew thinks is installed:

```bash
brew info ingeniousfrog/tap/mollow
```

If `mollow --version` is still behind the [latest release](https://github.com/ingeniousfrog/Mollow/releases/latest), reinstall:

```bash
brew uninstall mollow
brew update
brew install ingeniousfrog/tap/mollow
mollow --version
```

| Install method | Upgrade steps |
| --- | --- |
| **Homebrew** (macOS / Linux) | `brew update && brew upgrade mollow` |
| **Install scripts** | Re-run the script (defaults to the version baked into the script on `main`), or pin: `MOLLOW_VERSION=0.1.1 curl -fsSL …/install.sh \| bash` |
| **Windows PowerShell** | Re-run `install.ps1`, or `.\install.ps1 -Version 0.1.1` |
| **Manual download** | Download the new asset from [GitHub Releases](https://github.com/ingeniousfrog/Mollow/releases) and replace the binary on your `PATH` |
| **Build from source** | `git pull && cargo build --release -p mollow` |

---

### macOS

#### Option A — Homebrew (recommended)

Mollow is a CLI **Formula** (not a GUI Cask) in
[ingeniousfrog/homebrew-tap](https://github.com/ingeniousfrog/homebrew-tap):

```bash
brew tap ingeniousfrog/tap
brew install mollow
```

If Homebrew reports an untrusted tap, run once:

```bash
brew trust ingeniousfrog/tap
```

Upgrade / uninstall:

```bash
brew update          # refresh ingeniousfrog/tap first
brew upgrade mollow
brew uninstall mollow  # remove if needed
```

Supported architectures: Apple Silicon (`aarch64`) and Intel (`x86_64`).

#### Option B — Install script

Downloads the matching release tarball to `~/.local/bin` by default:

```bash
curl -fsSL https://raw.githubusercontent.com/ingeniousfrog/Mollow/main/packaging/install.sh | bash
```

Install system-wide:

```bash
MOLLOW_INSTALL_DIR=/usr/local/bin curl -fsSL https://raw.githubusercontent.com/ingeniousfrog/Mollow/main/packaging/install.sh | sudo bash
```

#### Option C — Manual download

Download `mollow-aarch64-apple-darwin.tar.gz` or `mollow-x86_64-apple-darwin.tar.gz` from
[GitHub Releases](https://github.com/ingeniousfrog/Mollow/releases), extract `mollow`, and place it on your `PATH`.

---

### Linux

Mollow is **not** packaged for `apt`, `dnf`, or official distro repos. Use one of the options below.

#### Option A — Install script (recommended)

Generic script for Linux x86_64 (and macOS). Default install path: `~/.local/bin`.

```bash
curl -fsSL https://raw.githubusercontent.com/ingeniousfrog/Mollow/main/packaging/install.sh | bash
```

System-wide:

```bash
MOLLOW_INSTALL_DIR=/usr/local/bin curl -fsSL https://raw.githubusercontent.com/ingeniousfrog/Mollow/main/packaging/install.sh | sudo bash
```

#### Option B — Ubuntu / Debian script

Dedicated script for Ubuntu/Debian x86_64. Default install path: `/usr/local/bin`.

```bash
curl -fsSL https://raw.githubusercontent.com/ingeniousfrog/Mollow/main/packaging/install-ubuntu.sh | sudo bash
```

User-local install (no `sudo`):

```bash
curl -fsSL https://raw.githubusercontent.com/ingeniousfrog/Mollow/main/packaging/install-ubuntu.sh -o install-ubuntu.sh
MOLLOW_INSTALL_DIR="$HOME/.local/bin" bash install-ubuntu.sh
```

Requires `curl` and `tar` (`sudo apt-get install -y curl` if missing).

#### Option C — Homebrew on Linux

```bash
brew tap ingeniousfrog/tap
brew install mollow
```

#### Option D — Manual download

Download `mollow-x86_64-unknown-linux-gnu.tar.gz` from
[GitHub Releases](https://github.com/ingeniousfrog/Mollow/releases), extract `mollow`, and place it on your `PATH`.

---

### Windows

#### Option A — PowerShell install script (recommended)

Installs to `%LOCALAPPDATA%\Programs\Mollow\bin` and adds it to the user `PATH`:

```powershell
irm https://raw.githubusercontent.com/ingeniousfrog/Mollow/main/packaging/install.ps1 | iex
```

Pin a specific version:

```powershell
irm https://raw.githubusercontent.com/ingeniousfrog/Mollow/main/packaging/install.ps1 -OutFile install.ps1
.\install.ps1 -Version 0.1.1
```

Restart the terminal after installation, then:

```powershell
mollow --version
```

#### Option B — Scoop

Manifest template: [`packaging/scoop/mollow.json`](packaging/scoop/mollow.json). Add it to your bucket, then:

```powershell
scoop install mollow
```

#### Option C — winget

Manifest template: [`packaging/winget/ingeniousfrog.Mollow.yaml`](packaging/winget/ingeniousfrog.Mollow.yaml).
Submit or adapt it for [microsoft/winget-pkgs](https://github.com/microsoft/winget-pkgs).

#### Option D — Manual download

Download `mollow-x86_64-pc-windows-msvc.zip` from
[GitHub Releases](https://github.com/ingeniousfrog/Mollow/releases), extract `mollow.exe`, and add its folder to `PATH`.

---

### Build from source

Requirements: Rust **1.85+** (`rust-version` in `Cargo.toml`).

```bash
git clone https://github.com/ingeniousfrog/Mollow.git
cd Mollow
cargo build --release -p mollow
./target/release/mollow --version
```

Add the binary to your `PATH`, or invoke via `cargo run --release -p mollow --`.

> Use a **release** build for performance baselines. Debug builds run but emit a
> comparability warning.

---

### Release assets and maintainer docs

| Asset | Platform |
| --- | --- |
| `mollow-aarch64-apple-darwin.tar.gz` | macOS Apple Silicon |
| `mollow-x86_64-apple-darwin.tar.gz` | macOS Intel |
| `mollow-x86_64-unknown-linux-gnu.tar.gz` | Linux x86_64 |
| `mollow-x86_64-pc-windows-msvc.zip` | Windows x86_64 |

Publishing a new version: push a `v*` tag (see [`.github/workflows/release.yml`](.github/workflows/release.yml)), then update checksums and the Homebrew tap:

```bash
./packaging/update-homebrew-sha256.sh <version>
./packaging/update-package-checksums.sh <version>
./packaging/push-homebrew-tap.sh
```

Further details:

| Document | Topic |
| --- | --- |
| [docs/packaging.md](docs/packaging.md) | All install paths, Scoop, winget |
| [docs/homebrew.md](docs/homebrew.md) | Homebrew Formula maintainer workflow |

---

## Command reference

### Shared flags

Most commands accept:

| Flag | Values | Default | Description |
| --- | --- | --- | --- |
| `--format` | `terminal`, `json`, `markdown`, `html` | see per-command | Output format |
| `--lang` | `english`, `zh-CN` | `english` | Report language (terminal / markdown / html) |
| `--output <PATH>` | file path | stdout | Write result to file instead of stdout |

Benchmark-related commands also accept:

| Flag | Values | Default | Description |
| --- | --- | --- | --- |
| `--profile` | `quick`, `standard` | `quick` | Sample count and input sizes ([details](docs/benchmarks.md)) |

---

### `mollow inspect`

Collect a **machine snapshot only** (no benchmarks).

```bash
mollow inspect [OPTIONS]
```

| Option | Default | Description |
| --- | --- | --- |
| `--format` | `terminal` | `terminal` · `json` · `markdown` · `html` |
| `--lang` | `english` | `english` · `zh-CN` |
| `--output` | — | Save rendered output to a file |

**Examples**

```bash
# Human-readable summary (Chinese labels)
mollow inspect --format terminal --lang zh-CN

# Machine-readable snapshot for tooling
mollow inspect --format json --output snapshot.json

# Shareable HTML report
mollow inspect --format html --lang english --output inspect.html
```

---

### `mollow bench`

Run benchmark workloads **without** saving a combined capture file (stdout or `--output`).

```bash
mollow bench [OPTIONS]
```

| Option | Default | Description |
| --- | --- | --- |
| `--profile` | `quick` | `quick` (3 samples) · `standard` (5 samples) |
| `--format` | `terminal` | Output format |
| `--lang` | `english` | Report language |
| `--output` | — | Output file path |

**Examples**

```bash
mollow bench --profile quick --format terminal
mollow bench --profile standard --format json --output bench-standard.json
```

---

### `mollow capture`

Snapshot **plus** benchmark in a **single JSON artifact** (recommended for baselines).

```bash
mollow capture [OPTIONS]
```

| Option | Default | Description |
| --- | --- | --- |
| `--profile` | `quick` | Benchmark profile |
| `--format` | `json` | Default is JSON for archival; use `terminal` for a quick read |
| `--lang` | `english` | Report language |
| `--output` | — | **Strongly recommended** — baseline file path |

**Examples**

```bash
mollow capture --profile quick --output baseline.json
mollow capture --profile standard --format json --output release-baseline.json
```

---

### `mollow compare`

Diff a **baseline** against one or more **candidates**.

Accepts:

- **Benchmark runs** (`started_at_unix_ms` present) → workload regression/improvement
- **Snapshots only** (`captured_at_unix_ms` only) → machine field changes, no workload deltas

```bash
mollow compare [OPTIONS] <BASELINE> <CANDIDATE> [MORE_CANDIDATES...]
```

| Argument / option | Description |
| --- | --- |
| `<BASELINE>` | Reference JSON file |
| `<CANDIDATE>` | File to compare against baseline |
| `[MORE_CANDIDATES...]` | Optional additional candidates in one invocation |
| `--format` | Default `terminal` |
| `--lang` | Report language |
| `--output` | Write comparison report to file |

**Examples**

```bash
mollow compare baseline.json candidate.json
mollow compare baseline.json run-a.json run-b.json --format markdown -o diff.md
mollow compare old-snapshot.json new-snapshot.json --lang zh-CN
```

**Comparability (summary)** — a benchmark diff is marked **not comparable** when schema,
profile, release build, workload parameters, or **environment** (power source, battery,
low-power mode, thermal warning/critical) differ. See [docs/comparison.md](docs/comparison.md).

Median workload change uses a **±5%** threshold (500 basis points) for
regression / improvement / stable classification.

---

### `mollow report`

Re-render any saved Mollow JSON (snapshot, benchmark, or comparison) to another format.

```bash
mollow report [OPTIONS] <INPUT>
```

| Option | Default | Description |
| --- | --- | --- |
| `<INPUT>` | — | `.json` file (auto-detected document type) |
| `--format` | `terminal` | Output format |
| `--lang` | `english` | Report language |
| `--output` | — | Output file (required for `html` when piping) |

**Examples**

```bash
mollow report baseline.json --format html --output report.html
mollow report comparison.json --format markdown --lang zh-CN
```

---

### `mollow watch`

Monitor **memory**, **power**, and **thermal** readings at a fixed interval (similar in
spirit to `gpustat -i 1`, but for environment state rather than GPU utilization).

```bash
mollow watch [OPTIONS]
```

| Option | Default | Description |
| --- | --- | --- |
| `-i`, `--interval` | `1` | Refresh interval in seconds |
| `--fields` | `memory,power,thermal` | Comma-separated fields to show |
| `--lang` | `english` | Report language |
| `--count` | — | Stop after N refresh cycles |

**Examples**

```bash
mollow watch -i 1
mollow watch -i 5 --fields power,thermal --lang zh-CN
```

Battery power and thermal warning/critical states are highlighted in the terminal.

---

### `mollow archive`

Manage a **local directory** of benchmark JSON files (index + trends).

#### `archive add`

```bash
mollow archive add --dir <ARCHIVE_DIR> <BENCHMARK.json>
```

Copies metadata into the archive index. Input must be a benchmark run (e.g. from `capture`).

#### `archive list`

```bash
mollow archive list --dir <ARCHIVE_DIR> [--format terminal|json|markdown|html] [--lang english|zh-CN]
```

#### `archive trend`

```bash
mollow archive trend --dir <ARCHIVE_DIR> --workload <NAME> [--format ...] [--lang ...]
```

`--workload` is one of: `cpu`, `memory`, `storage`, `gpu`, `media` (default: `cpu`).

**Examples**

```bash
mkdir -p ~/.mollow/archive
mollow capture --profile quick -o run-2025-06-19.json
mollow archive add --dir ~/.mollow/archive run-2025-06-19.json
mollow archive list --dir ~/.mollow/archive --format markdown
mollow archive trend --dir ~/.mollow/archive --workload gpu --lang zh-CN
```

---

## Benchmark profiles

| Profile | Samples | CPU input | Memory buffer | Storage file | Use case |
| --- | ---: | ---: | ---: | ---: | --- |
| `quick` | 3 | 4 MiB | 16 MiB | 8 MiB | Frequent local checks, CI smoke |
| `standard` | 5 | 32 MiB | 64 MiB | 64 MiB | Release baselines, archival |

Full warmup counts, statistics (median + MAD), and storage safety: [docs/benchmarks.md](docs/benchmarks.md).

---

## Platform support

| Platform | System / CPU / memory / storage | GPU | Media | Power | Thermal |
| --- | --- | --- | --- | --- | --- |
| macOS | Native APIs, sysctl | `system_profiler` | VideoToolbox | IOKit | SMC / thermal |
| Linux | `/proc`, sysfs | DRM | VA-API / V4L2 | power-supply | thermal zones |
| Windows | Win32 / NT | DXGI | Media Foundation | Win32 power | WMI |

---

## Schema versions

| Artifact | Schema | Path |
| --- | --- | --- |
| Machine snapshot | v3.0.0 | `schemas/machine-snapshot-v3.schema.json` |
| Benchmark run | v3.0.0 | `schemas/benchmark-run-v3.schema.json` |
| Comparison report | v2.0.0 | `schemas/comparison-report-v2.schema.json` |

---

## Development

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test --workspace --release
```

| Document | Topic |
| --- | --- |
| [docs/architecture.md](docs/architecture.md) | Crate boundaries, capability semantics |
| [docs/benchmarks.md](docs/benchmarks.md) | Workloads, profiles, statistics |
| [docs/comparison.md](docs/comparison.md) | Comparability and strict environment rules |
| [docs/release-verification.md](docs/release-verification.md) | Pre-release checklist |
| [docs/homebrew.md](docs/homebrew.md) | Formula packaging |
| [docs/packaging.md](docs/packaging.md) | Cross-platform install, Scoop, winget |

---

## License

Apache License 2.0 — see [`LICENSE`](LICENSE).
