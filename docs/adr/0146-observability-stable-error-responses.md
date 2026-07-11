# ADR 0146: Observability stable error responses

## Status

Accepted

## Context

`prodex-observability` owns trace propagation and gateway span planning.
`SpanPlanError` can reveal trace propagation details, metric-label validation
internals, or high-cardinality attribute names. Gateway-core previously
hardcoded telemetry failure responses wherever span planning was used.

## Decision

Add `plan_span_error_response` to `prodex-observability`. It maps raw
`SpanPlanError` values into the stable `telemetry_unavailable` response.

Gateway-core now adapts that observability response for admission, usage
reconciliation, and expired-reservation recovery telemetry failures.

## Consequences

Telemetry failure redaction is centralized at the observability boundary.
Gateway response planners can keep using gateway-specific status types without
inspecting raw span planning errors.
