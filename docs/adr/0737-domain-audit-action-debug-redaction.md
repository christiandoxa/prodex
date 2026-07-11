# ADR 0737: Redact domain audit action debug output

## Status

Accepted

## Context

Audit actions are stable machine-readable names. Exact values remain required
for durable audit records and authorization analysis, but generic `Debug` output
can surface through panic diagnostics or structured logs outside authorized
audit tooling.

## Decision

Implement custom `Debug` for `AuditAction`. Debug output redacts the exact
action string while preserving the type shape. Construction, validation,
serialization, and the explicit `as_str()` accessor are unchanged.

## Consequences

- Durable audit action values remain backward compatible.
- Generic diagnostics no longer expose raw action names.
- Authorized domain logic still reads exact action names through `as_str()`.
