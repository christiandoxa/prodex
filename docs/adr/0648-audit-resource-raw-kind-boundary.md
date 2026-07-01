# 0648: Audit Resource Raw Kind Boundary

## Status

Accepted

## Context

`AuditResource::try_new` validates stable machine-readable resource kinds, but
the legacy `AuditResource::new` constructor accepted raw strings. A caller could
persist request-controlled resource kinds containing whitespace, path-like text,
or credential-like material.

## Decision

`AuditResource::new` now stores the provided kind only when it passes the
resource-kind validator. Invalid raw kinds are replaced with the stable
low-cardinality kind `audit.invalid_resource`. Strict constructors validate the
raw kind before constructing so invalid inputs still fail closed.

## Consequences

- Audit records do not persist request-controlled resource kinds.
- Existing callers keep the same constructor signature.
- Invalid resource-kind values remain visible as a stable sentinel without
  leaking raw input.
