# ADR 0458: Runtime metrics do not expose executable paths

## Status

Accepted.

## Context

The runtime broker Prometheus `info` metric included `executable_path` as a
label. Absolute paths are environment-specific, potentially sensitive, and add
unnecessary metric cardinality. The executable digest is enough for metric-side
build identification.

## Decision

Remove `executable_path` from Prometheus labels. Keep the snapshot field
available for trusted diagnostics that render non-metric summaries.

## Consequences

Metrics no longer expose local filesystem layout or split series by deploy path.
