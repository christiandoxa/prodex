# ADR 0831: Redact domain health probe debug revision metadata

Status: Accepted

## Context

`HealthProbeResponsePlan` returns active policy revision metadata as a typed
field for callers, but derived `Debug` output exposed the raw revision
identifier through diagnostics.

## Decision

Use a custom `Debug` implementation for `HealthProbeResponsePlan` that preserves
probe state and stable response fields while redacting active policy revision
identity.

## Consequences

Health callers still receive the typed active revision value, but diagnostics no
longer expose it through health probe response debug output.
