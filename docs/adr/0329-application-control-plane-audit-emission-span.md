# ADR 0329: Application Control-Plane Audit Emission Span

## Status

Accepted.

## Context

Control-plane audit mutation now has an application correlation planner that
binds request ID, optional call ID, trace ID, tenant ID, and audit event ID.
Enterprise observability also requires a dedicated audit-emission span without
placing high-cardinality identifiers into metric labels. Composition roots need a
side-effect-free application planner that turns the canonical correlation
context into an observability span plan.

## Decision

Add `plan_application_control_plane_audit_emission_span` to
`prodex-application`.

The planner requires tenant and audit-event correlation, emits the canonical
`GatewaySpanKind::AuditEmission` span, and attaches tenant ID plus audit event ID
only as trace-scoped attributes. Failures are exposed through the existing
redacted telemetry-unavailable error envelope.

## Consequences

- Audit emission has a stable application-level span boundary.
- Tenant ID and audit event ID remain trace correlation data, not metric labels.
- Missing correlation fails before a composition root emits incomplete audit
  telemetry.
