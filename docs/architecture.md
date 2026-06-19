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
    +-- mollow-report ---- JSON and future terminal/Markdown/HTML renderers
```

- `mollow-core` owns stable, serializable domain types. It must not depend on a
  specific operating system or report format.
- `mollow-platform` owns collection interfaces and native adapters. Collection
  failures become explicit capability states instead of missing or fabricated
  values.
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
and an explicit migration path. Report language and formatting never change
the underlying snapshot.

## Current platform coverage

The first vertical slice uses `sysctlbyname` through a thin macOS FFI wrapper
for operating-system, CPU, and installed-memory facts. Other targets compile
with a conservative standard-library adapter and report unimplemented facts
as errors. Native Linux and Windows collectors are subsequent milestones.

