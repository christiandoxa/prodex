# ADR 0348: Observability Routing Decision Metric

## Status

Accepted.

## Context

Enterprise gateways need routing decision telemetry for responses, compact,
websocket, and control-plane lanes. These metrics must not label by tenant,
profile, request ID, model, provider endpoint, prompt, or raw routing error
text.

## Decision

Add `plan_routing_decision_metric` to `prodex-observability`.

The planner emits `prodex_routing_decisions_total`, increments by one, and
exposes only the closed enum labels `routing_lane` and `routing_outcome`.

## Consequences

- Gateway and control-plane adapters can publish route selection, fallback,
  rejection, and no-candidate counters through one shared contract.
- Routing labels remain low-cardinality and reviewable.
- Tenant IDs, profile names, request IDs, models, endpoints, prompts, and raw
  routing errors remain trace/log/audit concerns subject to redaction policy.
