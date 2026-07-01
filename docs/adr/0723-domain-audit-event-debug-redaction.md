# ADR 0723: Redact domain audit event debug output

## Status

Accepted

## Context

Audit events are immutable evidence and must keep their serialized tenant,
principal, resource, reason, timestamp, and event identifiers. Generic `Debug`
output is a different boundary: it can appear in panic diagnostics, test
failures, or structured logs outside the authorized audit-query path. Derived
debug output exposed those audit fields directly.

## Decision

Implement custom `Debug` for `AuditEvent`. Debug output keeps action, outcome,
and the redacted resource shape for low-cardinality diagnostics, while redacting
event ID, timestamp, tenant ID, principal ID, and reason code. Serialization and
equality remain unchanged.

## Consequences

- Durable audit records remain complete and queryable through authorized audit
  tooling.
- Generic logs no longer expose audit event identifiers, tenant/principal IDs,
  resource identifiers, timestamps, or reason codes.
- Exact audit evidence must come from the immutable audit store, not debug
  dumps.
