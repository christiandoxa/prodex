# ADR 0747: Redact domain audit retention batch-limit error debug output

## Status

Accepted

## Context

`AuditRetentionBatchLimit` redacts exact cleanup batch sizes in `Debug` output.
Its validation error still exposed rejected oversized batch limits through
derived `Debug`. Retention cleanup diagnostics should keep bounded-operation
shape without echoing operator-selected limits.

## Decision

Implement custom `Debug` for `AuditRetentionBatchLimitError`. Keep `Zero` as a
low-cardinality variant name and render `TooLarge.value` as `"<redacted>"`.

## Consequences

Diagnostics still identify the validation branch. Exact rejected batch sizes
remain available through typed control flow, not formatter output.
