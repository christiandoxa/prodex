# ADR 0374: Observability API Timeout Budget Metric

## Status

Accepted.

## Context

Enterprise HTTP data-plane hardening requires explicit timeout budgets across
gateway, control-plane, provider, and persistence work. Operators need to count
accepted, expired, exhausted, and cancelled timeout outcomes without exposing
request IDs, tenant IDs, route paths, raw deadlines, or exact durations as
metric labels.

## Decision

Add `plan_api_timeout_budget_metric` to `prodex-observability`.

The planner emits `prodex_api_timeout_budget_events_total`, increments by one,
and uses only the closed enum labels `api_timeout_budget_surface` and
`api_timeout_budget_result`.

## Consequences

- Gateway and control-plane adapters can publish timeout-budget enforcement
  outcomes through a shared low-cardinality contract.
- Raw deadlines, durations, tenants, request IDs, and route paths remain
  trace/log/report data subject to redaction.
- The observability boundary guard can enforce the timeout-budget telemetry
  contract before a concrete metrics backend is wired in.
