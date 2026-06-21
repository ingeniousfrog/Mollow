# Hardware enrichment

Mollow v4 adds optional offline hardware catalog enrichment for machine snapshots.

## Usage

```bash
mollow inspect --enrich
mollow inspect --enrich --format html --lang zh-CN
mollow capture --enrich --profile quick
```

Without `--enrich`, `hardware_context` is `unsupported` and snapshots stay minimal.

## What enrichment provides

- CPU/GPU/memory specifications from the embedded catalog in [`data/hardware/catalog.json`](../data/hardware/catalog.json)
- Architecture summaries and reference URLs
- Simplified architecture SVG diagrams in HTML reports
- Benchmark reference scores and optional local-vs-catalog deltas when `--enrich` is used with `capture`

Enrichment does **not** emit traditional tier-list ranks. Relative context uses catalog reference scores and basis-point deltas instead.

## Catalog coverage

Catalog version **2026.06** (`synthetic_reference_index` reference scores).

| Category | Examples in catalog | Notes |
| --- | --- | --- |
| CPU | Intel Core i7-12700K, AMD Ryzen 9 5950X, **Apple M1–M5** | Pro/Max/Ultra map to generation entry |
| GPU | GeForce RTX 3060/4060/4090, Radeon RX 6700 XT, **Apple M1–M5 GPU** | macOS integrated GPU name matches chip (e.g. `Apple M2`) |
| Memory | DDR4-3200, DDR5-5600, LPDDR5-6400 | Module fields depend on OS probing |

Apple Silicon GPU entries use the same match patterns as CPU generations because `system_profiler` reports the chip name, not a separate GPU marketing name.

Catalog updates ship with Mollow releases (offline-first; no runtime API). See [ADR-0002](adr/0002-hardware-enrichment-decisions.md).

## Design decisions

See [ADR-0002](adr/0002-hardware-enrichment-decisions.md).

## Schema

Machine snapshots use schema v4: [`schemas/machine-snapshot-v4.schema.json`](../schemas/machine-snapshot-v4.schema.json).
