# ADR 0722: Redact domain audit resource debug output

## Status

Accepted

## Context

Audit resources are intentionally serialized into immutable audit events, but
generic `Debug` output can also appear in panic diagnostics or structured logs
outside the audited storage path. Derived debug output printed resource IDs and
tenant IDs even though resource-kind and resource-id validation already protects
client-facing error envelopes.

## Decision

Implement custom `Debug` for `AuditResource` and `AuditResourceId`. Debug output
keeps the resource kind for low-cardinality diagnostics and redacts the resource
identifier and tenant identifier. Serialization and equality remain unchanged so
durable audit events still carry the validated resource metadata they require.

## Consequences

- Audit event serialization remains the authoritative evidence path.
- Generic debug logs no longer expose tenant IDs or resource identifiers.
- Operators who need exact resource metadata must query the audited store
  through authorized audit tooling.
