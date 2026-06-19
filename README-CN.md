# Mollow

[English](README.md) | 简体中文

Mollow 是一个跨平台的机器环境探测与性能基线 CLI，旨在回答三个问题：

1. 这台机器能做什么？
2. 它当前处于什么状态？
3. 与历史基线或另一台机器相比，发生了什么变化？

Mollow 收集硬件、操作系统与运行时信息，并配合轻量、可复现的真实场景负载。它并不试图替代完整的基准测试套件、硬件监控或调优工具。

## 当前里程碑

首个垂直切片提供带版本号的机器快照，包含：

- 通过原生 API 与薄 FFI 在 macOS 上采集系统、CPU 拓扑/特性、实时内存、挂载卷与已安装运行时；
- 显式的能力状态与采集来源；
- 稳定、格式化输出的 JSON；
- 针对受限观测项（如 swap 用量）的字段级状态；
- 对计划中 GPU、媒体、电源与温控采集器的明确占位，标记为不支持。

Linux 侧提供基于 `/proc`、`uname` 与 `statvfs` 的原生采集器，解析逻辑配有 fixture 测试。Windows 侧通过薄 Win32/NT FFI 采集系统、CPU、内存与卷信息。两个适配器均通过跨目标 Rust 检查；Windows 实机验证仍待完成。基准测试、对比与更多报告格式将在后续阶段加入。

## 构建与运行

在仓库根目录执行：

```bash
cargo build --workspace
cargo run -p mollow -- inspect --format terminal --lang zh-CN
cargo run --release -p mollow -- bench --profile quick --format json
cargo run --release -p mollow -- capture --output baseline.json
cargo run -p mollow -- compare baseline.json candidate.json --format markdown
cargo run -p mollow -- report baseline.json --format html --output report.html
```

运行质量检查：

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## 工作区结构

- `crates/mollow-core`：带版本号的领域模型与能力语义
- `crates/mollow-bench`：带版本号的 CPU、内存与存储负载
- `crates/mollow-compare`：可比性检查与字段级基线变化
- `crates/mollow-platform`：采集契约与原生适配器
- `crates/mollow-report`：中英双语 JSON、终端、Markdown 与 HTML 渲染
- `crates/mollow-cli`：命令行接口与应用协调
- `schemas`：带版本号的导出契约
- `docs`：架构说明与决策记录

设计边界与 schema 演进规则见[架构指南](docs/architecture.md)。在对比或归档性能结果前，请先阅读[基准测试方法论](docs/benchmarks.md)。

## 当前能力

- macOS：原生系统/CPU/内存/存储探测；通过 `system_profiler` 获取 GPU；VideoToolbox 硬件解码检测；电池/电源状态；系统在暴露时提供温控状态。
- Linux：`/proc`、sysfs、DRM、电源供应与 thermal zone 探测。
- Windows：Win32/NT 系统、CPU、内存、存储与电源探测；GPU、媒体与温控探测明确标记为不支持。
- 报告：支持英文或中文的 JSON、终端、Markdown 与自包含 HTML。
- 基线：采集、对比、机器/运行时变更检测，以及明确的不可比原因说明。
