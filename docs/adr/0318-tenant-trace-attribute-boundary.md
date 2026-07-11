# ADR 0318: Tenant Trace Attribute Boundary

## Status

Accepted.

## Context

Enterprise telemetry needs tenant correlation for incident investigation, but
raw tenant IDs are high-cardinality values and must not become metric labels.
`TelemetryAttribute::as_metric_label` already rejects tenant identifier keys,
but adapter code still needs an obvious safe path for carrying tenant context in
traces.

## Decision

Add `tenant_trace_attribute(tenant_id)` to `prodex-domain`. The helper returns a
`TelemetryAttribute` with key `tenant_id`, the canonical tenant identifier as
the value, and `TraceOnly` scope. Using it as a metric label fails with the same
trace-only validation error as other trace-only attributes.

`ci:domain-boundary-guard` checks that the helper stays trace-only and remains
exported from the domain crate.

## Consequences

Composition roots and observability adapters can include tenant correlation in
spans without copying raw tenant IDs into low-cardinality metric labels. Metric
label validation remains the enforcement point, while the helper gives callers a
safe default.
