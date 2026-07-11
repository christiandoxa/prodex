# ADR 0732: Redact domain audit time range debug output

## Status

Accepted

## Context

Audit time ranges carry query boundary timestamps. The exact values are required
for filtering and cursor-compatible pagination, but generic `Debug` output can
surface through panic diagnostics or structured logs outside authorized audit
tooling.

## Decision

Implement custom `Debug` for `AuditTimeRange`. Debug output preserves whether
start/end bounds are present while redacting exact timestamp values.
Construction, containment checks, serialization, and validation are unchanged.

## Consequences

- Audit query behavior remains backward compatible.
- Generic diagnostics no longer expose raw audit query time boundaries.
- Authorized audit query paths still retain exact start/end timestamps.
