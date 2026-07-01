# ADR 0733: Redact domain audit page limit debug output

## Status

Accepted

## Context

Audit page limits constrain query and export result size. Exact values remain
required for pagination behavior, but generic `Debug` output can surface through
panic diagnostics or structured logs outside authorized audit tooling.

## Decision

Implement custom `Debug` for `AuditPageLimit`. Debug output redacts the exact
limit value while preserving the type shape. Construction, ordering,
serialization, and the explicit `get()` accessor are unchanged.

## Consequences

- Audit query and export pagination remain backward compatible.
- Generic diagnostics no longer expose raw audit page-limit values.
- Authorized domain logic still reads the exact limit through `get()`.
