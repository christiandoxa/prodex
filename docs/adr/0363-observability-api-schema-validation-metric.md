# ADR 0363: Observability API Schema Validation Metric

## Status

Accepted.

## Context

Enterprise API governance requires request, response, OpenAPI, and stable
error-envelope validation telemetry. These metrics must not label by tenant ID,
raw route, schema path, field path, request ID, payload content, or raw
validation error text.

## Decision

Add `plan_api_schema_validation_metric` to `prodex-observability`.

The planner emits `prodex_api_schema_validation_total`, increments by one, and
exposes only the closed enum labels `api_schema_surface` and
`api_schema_result`.

## Consequences

- Gateway and control-plane validation adapters can publish schema validation
  pass/fail events through one shared contract.
- Schema validation labels remain low-cardinality and reviewable.
- Tenant IDs, raw routes, schema paths, field paths, request IDs, payloads, and
  raw validation errors remain trace/log/report concerns subject to redaction
  policy.
