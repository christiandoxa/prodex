# ADR 0359: Observability Load Soak Metric

## Status

Accepted.

## Context

Enterprise readiness requires load, soak, spike, and recovery tests with SLO
thresholds. These metrics must not label by tenant ID, profile, test run ID,
environment name, threshold value, request ID, prompt, or raw failure text.

## Decision

Add `plan_load_soak_metric` to `prodex-observability`.

The planner emits `prodex_load_soak_events_total`, records
`prodex_load_soak_duration_ms`, increments by one, and exposes only the closed
enum labels `load_soak_scenario` and `load_soak_result`.

## Consequences

- Load, soak, spike, and recovery test harnesses can publish pass/fail/abort
  events through one shared contract.
- Scenario and result labels remain low-cardinality and reviewable.
- Tenant IDs, profiles, run IDs, environments, threshold values, request IDs,
  prompts, and raw failure text remain trace/log/report concerns subject to
  redaction policy.
