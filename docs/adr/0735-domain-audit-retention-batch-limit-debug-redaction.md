# ADR 0735: Redact domain audit retention batch limit debug output

## Status

Accepted

## Context

Audit retention batch limits bound cleanup deletes. Exact values remain required
for deterministic retention cleanup, but generic `Debug` output can surface
through panic diagnostics or structured logs outside authorized audit tooling.

## Decision

Implement custom `Debug` for `AuditRetentionBatchLimit`. Debug output redacts
the exact limit value while preserving the type shape. Construction, ordering,
serialization, and the explicit `get()` accessor are unchanged.

## Consequences

- Audit retention cleanup behavior remains backward compatible.
- Generic diagnostics no longer expose raw retention batch-limit values.
- Authorized domain logic still reads the exact limit through `get()`.
