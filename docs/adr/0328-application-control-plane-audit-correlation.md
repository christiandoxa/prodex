# ADR 0328: Application Control-Plane Audit Correlation

## Status

Accepted.

## Context

Enterprise audit records must correlate request ID, optional call ID, trace ID,
tenant ID, and immutable audit event ID without turning high-cardinality values
into metric labels. Gateway HTTP already parses W3C `traceparent`, and the
control-plane audit planner already selects the immutable audit event, but
application composition roots still need a single side-effect-free boundary that
ties both together before audit mutation handling continues.

## Decision

Add `plan_application_control_plane_audit_correlation_from_http` to
`prodex-application`.

The planner reuses `plan_gateway_http_request` for HTTP policy and trace parsing,
then builds a domain `CorrelationContext` from the canonical request ID,
optional call ID, propagated trace ID, audit tenant partition, and immutable
audit event ID. Invalid or missing trace context is exposed through a stable
redacted application response planner.

## Consequences

- Control-plane audit handlers can carry one canonical correlation context.
- Trace propagation remains validated by the gateway HTTP boundary.
- Tenant, request, call, and audit event identifiers remain correlation data,
  not metric labels.
