# Release verification checklist

Run these checks on each supported platform before tagging a release.

## Automated checks

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test --workspace --release
cargo audit
```

## macOS

```bash
cargo run --release -p mollow -- inspect --format json --lang zh-CN
cargo run --release -p mollow -- bench --profile quick --format json
cargo run --release -p mollow -- capture --output /tmp/mollow-baseline.json
cargo run -p mollow -- compare /tmp/mollow-baseline.json /tmp/mollow-baseline.json --format markdown
cargo run -p mollow -- report /tmp/mollow-baseline.json --format html --output /tmp/mollow-report.html
cargo run -p mollow -- archive add /tmp/mollow-baseline.json --dir /tmp/mollow-archive
cargo run -p mollow -- archive list --dir /tmp/mollow-archive
```

Confirm:

- Snapshot schema version is `3.0.0`.
- Benchmark schema version is `3.0.0`.
- GPU, media, power, and thermal sections are `available` or explicitly
  `unsupported` / `unavailable` with a message.
- HTML report renders semantic sections, not only a single escaped text block.

## Linux

Repeat the macOS command sequence. Confirm DRM, VA-API/V4L2, power-supply, and
thermal-zone probes return explicit capability states.

## Windows

Repeat the macOS command sequence on a physical or VM Windows host. Confirm:

- DXGI GPU enumeration returns adapter names.
- Media Foundation reports hardware codec capabilities or explicit unsupported
  states.
- Power status is available.
- Thermal state is available through WMI or explicitly unavailable with a
  message.

## Comparison sanity

Capture two release baselines on the same machine with the same profile. The
comparison should be `comparable: true` and classify CPU, memory, and storage as
`stable` unless background load changed materially.

Capture a debug build baseline and compare it with a release baseline. The
comparison must be `comparable: false` with an explicit release-build reason.
