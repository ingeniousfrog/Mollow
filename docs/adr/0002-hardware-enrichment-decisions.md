# ADR-0002: Hardware enrichment product decisions

## Status

Accepted

## Context

Users asked whether Mollow should enrich machine snapshots with hardware tier rankings,
detailed specifications, and architecture diagrams. The enrichment layer must remain
compatible with ADR-0001 capability semantics and Mollow's core mission.

## Decision

1. **Primary audience** remains CI, regression checking, and baseline tracking users.
   Enrichment is an optional human-readable layer; it does not replace reproducible
   local benchmarks or strict environment diffs.

2. **Offline-first catalog** ships with Mollow releases. No online API is required for
   enrichment. Catalog updates follow release cadence.

3. **Specifications first**: codename, process node, cache, clocks, memory type, and
   architecture summaries are the default enrichment output.

4. **Relative performance context** uses catalog reference scores and optional local
   benchmark comparison (median rate vs catalog median). Traditional tier-list ranks
   are not emitted by default.

5. **Architecture diagrams** are limited to simplified, catalog-defined SVG templates
   plus optional reference URLs. Official vendor diagrams are not bundled.

6. **`inspect --enrich`** opt-in flag attaches `hardware_context` to snapshot v4.
   Without the flag, `hardware_context` is `unsupported`.

## Consequences

Positive:

- Enrichment stays explainable and auditable.
- Offline operation preserves CI and air-gapped usability.
- Scope remains aligned with environment audit rather than hardware shopping.

Negative:

- Catalog coverage is finite and requires ongoing curation.
- Tier-list expectations from consumer audiences are not fully met by design.

## Alternatives considered

- Online tier-list APIs: rejected for offline/air-gapped requirements and licensing risk.
- Default-on enrichment: rejected to keep inspect fast and snapshots minimal by default.
- Full tier-list mirroring: rejected as opinionated and high-maintenance.
