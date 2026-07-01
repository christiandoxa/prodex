# ADR 0347: Observability Security Decision Metric

## Status

Accepted.

## Context

Enterprise gateways need authentication, tenant resolution, authorization, and
credential-scope decision telemetry. These metrics must not label by tenant,
principal, role, claim value, token, key ID, request ID, or policy internals.

## Decision

Add `plan_security_decision_metric` to `prodex-observability`.

The planner emits `prodex_security_decisions_total`, increments by one, and
exposes only the closed enum labels `security_decision` and `security_result`.

## Consequences

- Gateway and control-plane adapters can publish security decision counters
  through one shared contract.
- Decision labels remain low-cardinality and reviewable.
- Tenant IDs, principals, roles, claim values, tokens, key IDs, request IDs, and
  policy internals remain trace/log/audit concerns subject to redaction policy.
