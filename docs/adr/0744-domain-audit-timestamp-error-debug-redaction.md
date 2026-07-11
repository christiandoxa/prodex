# ADR 0744: Redact domain audit timestamp error debug output

## Status

Accepted

## Context

Audit timestamp validation errors can include rejected millisecond values.
Client-facing response planners already avoid echoing those details, but
generic `Debug` output can surface through panic diagnostics or structured logs
outside authorized audit tooling.

## Decision

Implement custom `Debug` for `AuditTimestampError`. Debug output preserves the
variant shape while redacting the exact rejected timestamp. Validation,
equality, `Display`, and response planning are unchanged.

## Consequences

- Audit timestamp validation behavior remains backward compatible.
- Generic diagnostics no longer expose rejected timestamp values.
- Machine-readable response planning continues to use the same variants.
