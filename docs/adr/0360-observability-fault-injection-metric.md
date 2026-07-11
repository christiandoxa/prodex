# ADR 0360: Observability Fault Injection Metric

## Status

Accepted.

## Context

Enterprise readiness requires fault injection for PostgreSQL, Redis, IdP, and
provider dependencies. These metrics must not label by tenant ID, backend
endpoint, provider account, test run ID, request ID, prompt, or raw injected
error text.

## Decision

Add `plan_fault_injection_metric` to `prodex-observability`.

The planner emits `prodex_fault_injection_events_total`, increments by one, and
exposes only the closed enum labels `fault_injection_target` and
`fault_injection_result`.

## Consequences

- Fault-injection harnesses can publish inject, recover, fail, and skip events
  through one shared contract.
- Dependency target and result labels remain low-cardinality and reviewable.
- Tenant IDs, endpoints, provider accounts, run IDs, request IDs, prompts, and
  raw injected errors remain trace/log/report concerns subject to redaction
  policy.
