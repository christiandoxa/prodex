# ADR 0344: Observability Structured Log Correlation

## Status

Accepted.

## Context

Enterprise operations require structured logs that correlate `RequestId`,
`CallId`, trace ID, tenant, and audit event data. These identifiers are
high-cardinality and must not become metric labels, but adapters still need a
shared contract for including them in redacted traces and logs.

## Decision

Add `plan_structured_log_correlation` to `prodex-observability`.

The planner accepts a `CorrelationContext` and emits trace/log-only
`TelemetryAttribute` fields for request, call, trace, tenant, and audit event
identifiers when they are available. No field is emitted as a metric label.

## Consequences

- Gateway and control-plane adapters can attach consistent correlation fields to
  structured logs.
- High-cardinality identifiers remain available for incident reconstruction.
- Request, call, tenant, and audit identifiers stay out of metric labels and
  remain subject to log redaction and retention policy.
