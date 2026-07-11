# ADR 0739: Redact domain audit action error debug output

## Status

Accepted

## Context

Audit action validation errors can include metadata about rejected input, such
as raw length. Client-facing response planners already avoid echoing those
details, but generic `Debug` output can surface through panic diagnostics or
structured logs outside authorized audit tooling.

## Decision

Implement custom `Debug` for `AuditActionError`. Debug output preserves the
variant shape while redacting the exact rejected length. Validation, equality,
`Display`, and response planning are unchanged.

## Consequences

- Audit action validation behavior remains backward compatible.
- Generic diagnostics no longer expose rejected audit action lengths.
- Machine-readable response planning continues to use the same variants.
