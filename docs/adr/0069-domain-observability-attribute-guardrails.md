# ADR 0069: Domain observability attribute guardrails

## Status

Accepted.

## Context

The enterprise observability target requires spans for authentication, tenant
resolution, authorization, budget reservation, routing, provider calls,
streaming, persistence, reconciliation, and audit emission. It also requires
avoiding raw tenant IDs, user IDs, keys, prompts, request IDs, and other
high-cardinality values as metric labels. Existing gateway telemetry is mostly
custom logging and metrics; the pure domain layer did not define a common span or
attribute safety model.

## Decision

Add pure domain observability primitives: `GatewaySpanKind`,
`GatewaySpanDescriptor`, `TelemetryAttribute`, and metric-label validation.
Metric labels are allowed only for explicitly low-cardinality attributes and are
rejected when their keys identify raw tenant, user, principal, key, prompt,
request, or call identifiers. Trace-only and redacted trace-only attributes can
carry correlation data without becoming metric labels.

## Consequences

- Future OpenTelemetry integration can map domain span kinds to native spans
  without duplicating the required data-plane lifecycle vocabulary.
- Metrics code gets a reusable guardrail for label cardinality and sensitive
  identifier leakage.
- The domain crate remains free of OpenTelemetry SDK, HTTP, database, filesystem,
  and provider dependencies.
