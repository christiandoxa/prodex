# 0646: Audit Resource Raw ID Boundary

## Status

Accepted

## Context

`AuditResourceId` validates stable audit resource identifiers, but the legacy
`AuditResource::new` constructor accepted raw optional IDs. A caller could
accidentally persist a request-controlled resource ID containing whitespace,
path-like text, or credential-like material.

## Decision

`AuditResource::new` now stores a raw resource ID only when it passes the
`AuditResourceId` validator. Invalid raw IDs are dropped. The typed
`AuditResource::new_with_resource_id` constructor remains the strict path for
new callers.

## Consequences

- Audit records keep bounded printable resource IDs.
- Existing callers keep the same constructor signature.
- Invalid raw resource IDs do not become durable audit metadata.
