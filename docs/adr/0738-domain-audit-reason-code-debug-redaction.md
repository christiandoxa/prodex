# ADR 0738: Redact domain audit reason code debug output

## Status

Accepted

## Context

Audit reason codes explain security-sensitive outcomes such as denials and
retention holds. Exact values remain required for durable audit records and
authorized analysis, but generic `Debug` output can surface through panic
diagnostics or structured logs outside authorized audit tooling.

## Decision

Implement custom `Debug` for `AuditReasonCode`. Debug output redacts the exact
reason code while preserving the type shape. Construction, validation,
serialization, and the explicit `as_str()` accessor are unchanged.

## Consequences

- Durable audit reason code values remain backward compatible.
- Generic diagnostics no longer expose raw reason codes.
- Authorized domain logic still reads exact reason codes through `as_str()`.
