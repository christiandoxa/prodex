# ADR 0736: Redact domain audit timestamp debug output

## Status

Accepted

## Context

Audit timestamps appear in event occurrence, query cursor, time range, and
retention cleanup boundaries. Exact millisecond values remain required for
ordering and cutoff calculations, but generic `Debug` output can surface through
panic diagnostics or structured logs outside authorized audit tooling.

## Decision

Implement custom `Debug` for `AuditTimestamp`. Debug output redacts the exact
millisecond value while preserving the type shape. Construction, ordering,
serialization, and the explicit `unix_ms()` accessor are unchanged.

## Consequences

- Audit ordering, pagination, and retention behavior remain backward compatible.
- Generic diagnostics no longer expose raw audit timestamp values.
- Authorized domain logic still reads the exact timestamp through `unix_ms()`.
