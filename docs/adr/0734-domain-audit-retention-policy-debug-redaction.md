# ADR 0734: Redact domain audit retention policy debug output

## Status

Accepted

## Context

Audit retention policies carry cleanup windows. Exact day counts remain required
for retention cutoff calculation, but generic `Debug` output can surface through
panic diagnostics or structured logs outside authorized audit tooling.

## Decision

Implement custom `Debug` for `AuditRetentionPolicy`. Debug output redacts the
exact day count while preserving the type shape. Construction, ordering,
serialization, and the explicit `days()` accessor are unchanged.

## Consequences

- Audit retention behavior remains backward compatible.
- Generic diagnostics no longer expose raw retention policy windows.
- Authorized domain logic still reads the exact day count through `days()`.
