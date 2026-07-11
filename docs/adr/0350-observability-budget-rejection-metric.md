# ADR 0350: Observability Budget Rejection Metric

## Status

Accepted.

## Context

Enterprise gateways need explicit budget rejection telemetry for tenant budget,
virtual-key budget, rate-limit, reservation availability, and policy-denial
cases. These metrics must not label by tenant ID, virtual key ID, request ID,
prompt, policy revision, raw rejection text, or caller identity.

## Decision

Add `plan_budget_rejection_metric` to `prodex-observability`.

The planner emits `prodex_budget_rejections_total`, increments by one, and
exposes only the closed enum label `budget_rejection_reason`.

## Consequences

- Gateway adapters can publish budget rejection counters through one shared
  contract before provider invocation.
- Budget rejection labels remain low-cardinality and reviewable.
- Tenant IDs, virtual key IDs, request IDs, prompts, policy revisions, raw
  rejection text, and caller identities remain trace/log/audit concerns subject
  to redaction policy.
