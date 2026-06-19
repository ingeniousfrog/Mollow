# ADR-0001: Versioned snapshots and explicit capability states

## Status

Accepted

## Context

Machine information differs by operating system, hardware generation,
permissions, and runtime conditions. A nullable field cannot distinguish an
unsupported capability from a transient failure or an unimplemented adapter.
Long-lived baselines also need to remain interpretable after Mollow evolves.

## Decision

Mollow stores observations in a generic capability envelope containing a
status, optional value, data source, and optional diagnostic message. Every
snapshot includes independent schema and application versions.

## Consequences

Positive:

- Missing data remains explainable.
- Reports can expose collection provenance.
- Historical files have an explicit compatibility contract.
- Platform adapters cannot silently substitute guessed values.

Negative:

- Snapshot files are more verbose.
- Consumers must inspect status before reading values.
- Schema changes require deliberate migration work.

## Alternatives considered

- Plain nullable fields: smaller, but ambiguous and unsuitable for diagnosis.
- Platform-specific output models: easier initially, but prevents meaningful
  cross-machine comparison.
- A single application version: insufficient when schema compatibility and
  executable releases evolve independently.

