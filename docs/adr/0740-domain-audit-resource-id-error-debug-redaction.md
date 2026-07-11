# ADR 0740: Redact domain audit resource ID error debug output

## Status

Accepted

## Context

Audit resource ID validation errors can include metadata about rejected input,
such as raw length. Client-facing response planners already avoid echoing those
details, but generic `Debug` output can surface through panic diagnostics or
structured logs outside authorized audit tooling.

## Decision

Implement custom `Debug` for `AuditResourceIdError`. Debug output preserves the
variant shape while redacting the exact rejected length. Validation, equality,
`Display`, and response planning are unchanged.

## Consequences

- Audit resource ID validation behavior remains backward compatible.
- Generic diagnostics no longer expose rejected resource ID lengths.
- Machine-readable response planning continues to use the same variants.
