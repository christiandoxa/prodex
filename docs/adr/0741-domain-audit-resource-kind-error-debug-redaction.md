# ADR 0741: Redact domain audit resource kind error debug output

## Status

Accepted

## Context

Audit resource kind validation errors can include metadata about rejected input,
such as raw length. Client-facing response planners already avoid echoing those
details, but generic `Debug` output can surface through panic diagnostics or
structured logs outside authorized audit tooling.

## Decision

Implement custom `Debug` for `AuditResourceKindError`. Debug output preserves
the variant shape while redacting the exact rejected length. Validation,
equality, `Display`, and response planning are unchanged.

## Consequences

- Audit resource kind validation behavior remains backward compatible.
- Generic diagnostics no longer expose rejected resource kind lengths.
- Machine-readable response planning continues to use the same variants.
