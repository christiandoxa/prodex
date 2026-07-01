# ADR 0393: Observability Budget Policy Lifecycle Metric

## Status

Accepted.

## Context

Budget policy updates are high-impact control-plane mutations because they govern tenant and credential spending limits. Operators need to count update, scope validation, persistence, authorization, denial, and failure outcomes without exposing tenant IDs, budget scope identifiers, limit amounts, request payloads, caller identity, or raw storage errors as metric labels.

## Decision

Add `plan_budget_policy_lifecycle_metric` to `prodex-observability`.

The planner emits `prodex_budget_policy_lifecycle_events_total`, increments by one, and uses only the closed enum labels `budget_policy_operation` and `budget_policy_result`.

## Consequences

- Budget policy adapters can publish update, validation, persistence, denial, and failure outcomes through one shared low-cardinality contract.
- Tenant IDs, budget scope identifiers, limit amounts, request payloads, caller identity, and raw storage errors remain trace/log/audit data subject to redaction.
- The observability boundary guard can enforce the budget policy lifecycle telemetry contract before a concrete metrics backend is wired in.
