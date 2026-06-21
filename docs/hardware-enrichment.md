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

## Design decisions

See [ADR-0002](adr/0002-hardware-enrichment-decisions.md).

## Schema

Machine snapshots use schema v4: [`schemas/machine-snapshot-v4.schema.json`](../schemas/machine-snapshot-v4.schema.json).
