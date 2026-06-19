# Mollow

[English](README.md) | 简体中文

Mollow 是一个跨平台的机器环境探测与性能基线 CLI，旨在回答三个问题：

1. 这台机器能做什么？
2. 它当前处于什么状态？
3. 与历史基线或另一台机器相比，发生了什么变化？

Mollow 收集硬件、操作系统与运行时信息，并配合轻量、可复现的真实场景负载。它并不试图替代完整的基准测试套件、硬件监控或调优工具。

## 当前里程碑

首个完整垂直切片已落地，包含：

- 带版本号的机器快照（Schema v3）与性能基线（Benchmark Schema v3）；
- 通过原生 API 与薄 FFI 采集系统、CPU、内存、存储与运行时；
- macOS / Linux 上的 GPU、媒体、电源与温控探测；Windows 上的 DXGI GPU、Media Foundation 编解码能力与电源探测；
- 轻量 CPU、内存、存储、GPU 计算与媒体帧处理基准测试；
- `capture`、`compare`、本地 `archive` 档案，以及中英双语 JSON、终端、Markdown 与语义化 HTML 报告。

## 构建与运行

在仓库根目录执行：

```bash
cargo build --workspace
cargo run -p mollow -- inspect --format terminal --lang zh-CN
cargo run --release -p mollow -- bench --profile quick --format json
cargo run --release -p mollow -- capture --output baseline.json
cargo run -p mollow -- compare baseline.json candidate.json --format markdown
cargo run -p mollow -- report baseline.json --format html --output report.html
cargo run -p mollow -- archive add baseline.json --dir ~/.mollow/archive
cargo run -p mollow -- archive list --dir ~/.mollow/archive
cargo run -p mollow -- archive trend --dir ~/.mollow/archive --workload cpu
```

运行质量检查：

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test --workspace --release
```

## 工作区结构

- `crates/mollow-core`：带版本号的领域模型与能力语义
- `crates/mollow-bench`：带版本号的 CPU、内存、存储、GPU 与媒体负载
- `crates/mollow-compare`：可比性检查、环境警告与字段级基线变化
- `crates/mollow-platform`：采集契约与原生适配器
- `crates/mollow-report`：中英双语 JSON、终端、Markdown 与 HTML 渲染
- `crates/mollow-archive`：本地基线档案索引与趋势
- `crates/mollow-cli`：命令行接口与应用协调
- `schemas`：带版本号的导出契约
- `docs`：架构说明与决策记录

设计边界与 schema 演进规则见[架构指南](docs/architecture.md)。在对比或归档性能结果前，请先阅读[基准测试方法论](docs/benchmarks.md)与[发布前验证清单](docs/release-verification.md)。

## 当前能力

- macOS：原生系统/CPU/内存/存储探测；`system_profiler` GPU；VideoToolbox 硬件编解码检测；电池/电源；温控状态。
- Linux：`/proc`、sysfs、DRM、VA-API/V4L2 媒体能力、电源供应与 thermal zone 探测。
- Windows：Win32/NT 系统、CPU、内存、存储、DXGI GPU、Media Foundation 编解码、电源与 WMI 温控探测。
- 基准：CPU FNV-1a、内存顺序拷贝、存储顺序写读、GPU 矩阵乘、媒体帧字节处理。
- 报告：支持英文或中文的 JSON、终端、Markdown 与语义化自包含 HTML。
- 基线：采集、对比、档案管理、机器/运行时/环境变更检测，以及明确的不可比原因说明。
