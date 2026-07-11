# ADR 0743: Redact domain audit digest error debug output

## Status

Accepted

## Context

Audit digest validation errors can include metadata about rejected digest input,
such as raw length. Client-facing response planners already avoid echoing those
details, but generic `Debug` output can surface through panic diagnostics or
structured logs outside authorized audit tooling.

## Decision

Implement custom `Debug` for `AuditDigestError`. Debug output preserves the
variant shape while redacting the exact rejected length. Validation, equality,
and response planning are unchanged.

## Consequences

- Audit digest validation behavior remains backward compatible.
- Generic diagnostics no longer expose rejected digest lengths.
- Machine-readable response planning continues to use the same variants.
