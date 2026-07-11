# 0645: Audit Event Raw Reason Code Boundary

## Status

Accepted

## Context

`AuditReasonCode` validates stable machine-readable reason codes, but the
legacy `AuditEvent::new` constructor accepted raw optional strings. A caller
could accidentally persist a request-controlled reason containing whitespace,
path-like text, or secret-like material.

## Decision

`AuditEvent::new` now stores a raw reason code only when it passes the
`AuditReasonCode` validator. Invalid raw values are dropped. The typed
`AuditEvent::new_with_reason_code` constructor remains the strict path for new
callers.

## Consequences

- Audit records keep stable low-cardinality reason codes.
- Existing callers keep the same constructor signature.
- Invalid raw reasons do not become durable audit metadata.
