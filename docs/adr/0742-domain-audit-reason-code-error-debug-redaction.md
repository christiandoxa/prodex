# ADR 0742: Redact domain audit reason code error debug output

## Status

Accepted

## Context

Audit reason code validation errors can include metadata about rejected input,
such as raw length. Client-facing response planners already avoid echoing those
details, but generic `Debug` output can surface through panic diagnostics or
structured logs outside authorized audit tooling.

## Decision

Implement custom `Debug` for `AuditReasonCodeError`. Debug output preserves the
variant shape while redacting the exact rejected length. Validation, equality,
`Display`, and response planning are unchanged.

## Consequences

- Audit reason code validation behavior remains backward compatible.
- Generic diagnostics no longer expose rejected reason code lengths.
- Machine-readable response planning continues to use the same variants.
