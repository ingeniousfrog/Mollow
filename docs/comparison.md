# Baseline comparison

`mollow compare baseline.json candidate.json` compares complete benchmark runs,
not isolated scores.

A performance result is comparable only when:

- both files use the same benchmark schema;
- both use the same quick or standard profile;
- both were produced by release builds;
- workload IDs, versions, measurements, and parameters match.

Comparable workload medians are classified with a default threshold of 500
basis points (5%):

- at or below -5%: regression;
- at or above +5%: improvement;
- otherwise: stable.

The report separately lists changes in operating system, CPU identity and core
count, installed and available memory, GPU/API identity, power mode, thermal
state, and runtime versions. Such changes do not silently alter the percentage;
they remain visible context for interpreting it.

Machine state can vary between captures. Mollow therefore preserves raw
samples, MAD-based variation, warnings, and both embedded machine snapshots.
