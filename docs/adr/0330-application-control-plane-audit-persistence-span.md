# ADR 0330: Application Control-Plane Audit Persistence Span

## Status

Accepted.

## Context

Control-plane audit emission has a dedicated span, but the append-only storage
write also needs observability. Enterprise telemetry requirements call for a
persistence span while avoiding high-cardinality metric labels such as tenant ID,
request ID, call ID, and audit event ID.

## Decision

Add `plan_application_control_plane_audit_persistence_span` to
`prodex-application`.

The planner requires tenant and audit-event correlation, emits
`GatewaySpanKind::Persistence`, derives a low-cardinality `storage_backend`
metric label from the typed audit storage plan, and carries tenant ID plus audit
event ID only as trace-scoped attributes.

## Consequences

- Append-only audit storage has an application-level persistence span boundary.
- Storage backend remains a safe metric label because it is derived from typed
  storage plans, not request input.
- Tenant and audit event identifiers remain trace correlation data, not metric
  labels.
