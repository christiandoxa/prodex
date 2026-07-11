# ADR 0351: Observability Rate-Limit Decision Metric

## Status

Accepted.

## Context

Enterprise gateways need rate-limit decision telemetry for tenant, virtual-key,
principal, and provider scopes. These metrics must not label by tenant ID,
virtual key ID, principal ID, provider endpoint, request ID, retry key, prompt,
or raw limiter/storage error text.

## Decision

Add `plan_rate_limit_decision_metric` to `prodex-observability`.

The planner emits `prodex_rate_limit_decisions_total`, increments by one, and
exposes only the closed enum labels `rate_limit_scope` and
`rate_limit_decision`.

## Consequences

- Gateway adapters can publish allow, delay, reject, and unavailable decisions
  through one shared contract.
- Rate-limit labels remain low-cardinality and reviewable.
- Tenant IDs, virtual key IDs, principal IDs, provider endpoints, request IDs,
  retry keys, prompts, and raw limiter/storage errors remain trace/log/audit
  concerns subject to redaction policy.
