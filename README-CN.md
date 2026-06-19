# Mollow

[English](README.md) | 简体中文

Mollow 是一个跨平台的机器环境探测与性能基线 CLI，用于回答三个实际问题：

1. **能力** — 这台机器具备哪些硬件与运行时栈？
2. **状态** — 当前运行环境如何（电源、温控、内存压力）？
3. **变化** — 与已保存基线或另一台机器相比，发生了什么变化？

Mollow 将带版本号的机器快照与轻量、可复现的负载相结合，适用于回归检查与环境审计，并不试图替代专业基准套件、硬件监控或调优工具。

## 快速开始

环境要求：Rust **1.85+**（见 `Cargo.toml` 中的 `rust-version`）。

```bash
cargo build --workspace --release

# 查看当前机器状态
cargo run --release -p mollow -- inspect --format terminal --lang zh-CN

# 运行快速基准（可比基线请使用 --release）
cargo run --release -p mollow -- bench --profile quick --format json

# 采集、对比与报告
cargo run --release -p mollow -- capture --output baseline.json
cargo run -p mollow -- compare baseline.json candidate.json --format markdown
cargo run -p mollow -- report baseline.json --format html --output report.html

# 本地档案与趋势
cargo run -p mollow -- archive add baseline.json --dir ~/.mollow/archive
cargo run -p mollow -- archive list --dir ~/.mollow/archive
cargo run -p mollow -- archive trend --dir ~/.mollow/archive --workload cpu
```

## 命令一览

| 命令 | 用途 |
| --- | --- |
| `inspect` | 采集带版本号的机器快照（Schema v3） |
| `bench` | 运行 CPU、内存、存储、GPU、媒体负载（Benchmark Schema v3） |
| `capture` | 快照与基准测试合并为单一产物 |
| `compare` | 对比两次采集/基准，并应用可比性规则 |
| `report` | 输出 JSON、终端、Markdown 或语义化 HTML（中/英） |
| `archive` | 本地基线档案的添加、列表与趋势 |

## 负载（v2）

| 领域 | Workload ID | 后端 |
| --- | --- | --- |
| CPU | `cpu.fnv1a-stream` | 主机端 FNV-1a 确定性输入哈希 |
| 内存 | `memory.sequential-copy` | 主机端顺序 `copy_from_slice` |
| 存储 | `storage.sequential-write-read` | 临时文件写入、`sync_all`、校验读回 |
| GPU | `gpu.wgpu-matrix-multiply` | **wgpu** 计算着色器（Metal / Vulkan / DX12） |
| 媒体（macOS） | `media.videotoolbox-h264-encode` | **VideoToolbox** 硬件 H.264 编码 |
| 媒体（Windows） | `media.media-foundation-h264-decode` | **Media Foundation** 硬件 H.264 解码 |
| 媒体（Linux） | `media.vaapi-h264-decode` | **VA-API** 硬件 H.264 解码 |

配置档：`quick`（3 次采样、较小输入）与 `standard`（5 次采样、较大输入）。
预热次数、统计方法与存储安全说明见[基准测试方法论](docs/benchmarks.md)。

## 平台探测能力

| 平台 | 系统 / CPU / 内存 / 存储 | GPU | 媒体编解码 | 电源 | 温控 |
| --- | --- | --- | --- | --- | --- |
| macOS | 原生 API | `system_profiler` | VideoToolbox | IOKit 电池 | SMC / 温控 |
| Linux | `/proc`、sysfs | DRM | VA-API / V4L2 | power-supply sysfs | thermal zone |
| Windows | Win32 / NT | DXGI | Media Foundation | Win32 电源 | WMI |

无法探测的项会显式标记为 `unsupported` / `error`，不会根据设备名称推断。

## 对比规则

基准对比仅在以下条件同时满足时标记为**可比**：

- 两份文件使用相同的 benchmark schema、profile 与 release 构建；
- workload ID、版本、度量单位与参数一致；
- **运行环境稳定** — 见下方严格规则。

中位数变化按默认阈值 **500 基点（5%）** 分类：回归（≤ −5%）、提升（≥ +5%）或稳定。

### 严格环境模式

以下情况会将对比标记为**不可比**（除 schema/profile 不匹配外）：

- 基线与候选的电源来源不一致；
- 任一侧在**电池供电**下采集；
- 任一侧启用了**低电量模式**；
- 任一侧温控状态为 **warning** 或 **critical**。

`environment_warnings` 仍会在终端、Markdown 与 HTML 中高亮显示。
机器字段变化（操作系统、CPU、内存、GPU、运行时）单独列出，不会静默改变百分比结论。

详见[基线对比说明](docs/comparison.md)。

## 工作区结构

| Crate / 路径 | 职责 |
| --- | --- |
| `crates/mollow-core` | 带版本号的领域模型与能力语义 |
| `crates/mollow-platform` | 原生采集适配器（macOS / Linux / Windows） |
| `crates/mollow-bench` | 带版本号负载，含 wgpu GPU 与平台媒体硬解 |
| `crates/mollow-compare` | 可比性检查、严格环境规则与字段级差异 |
| `crates/mollow-report` | 中英双语 JSON、终端、Markdown 与 HTML 渲染 |
| `crates/mollow-archive` | 本地基线档案索引与趋势 |
| `crates/mollow-cli` | 命令行接口与应用协调 |
| `schemas/` | JSON Schema 导出契约（snapshot v3、benchmark v3、comparison v2） |
| `docs/` | 架构、基准、对比与发布验证文档 |

## 质量检查

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test --workspace --release
```

发布或归档性能基线前，请遵循[发布前验证清单](docs/release-verification.md)。

## 文档

- [架构指南](docs/architecture.md) — crate 边界与 schema 演进
- [基准测试方法论](docs/benchmarks.md) — 配置档、统计与负载参数
- [基线对比说明](docs/comparison.md) — 可比性与严格环境规则
- [发布前验证清单](docs/release-verification.md) — 发布前人工检查项

## 许可证

MIT — 见 `LICENSE`。
