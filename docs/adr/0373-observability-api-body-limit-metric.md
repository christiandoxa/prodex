# ADR 0373: Observability API Body Limit Metric

## Status

Accepted.

## Context

Enterprise HTTP data-plane hardening requires request body limits. Operators
need to count accepted, rejected-too-large, unknown-length, and truncated body
outcomes without exposing raw content lengths, route paths, tenant IDs, request
IDs, or payload details as metric labels.

## Decision

Add `plan_api_body_limit_metric` to `prodex-observability`.

The planner emits `prodex_api_body_limit_events_total`, increments by one, and
uses only the closed enum labels `api_body_limit_surface` and
`api_body_limit_result`.

## Consequences

- Gateway and control-plane HTTP adapters can publish body-limit enforcement
  outcomes through a shared low-cardinality contract.
- Raw content lengths, paths, tenants, request IDs, and payload details remain
  trace/log/report data subject to redaction.
- The observability boundary guard can enforce the body-limit telemetry contract
  before a concrete metrics backend is wired in.
