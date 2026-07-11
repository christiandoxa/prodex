# ADR 0260: Budget Policy Update Storage Composition

## Status

Accepted

## Context

Reservation accounting already uses tenant-scoped counters, but control-plane
budget updates did not yet have durable budget policy storage. Without a
storage and application boundary, adapters could update budget limits outside
authorization/audit composition or duplicate backend-specific SQL.

## Decision

Add `BudgetPolicyUpdateCommand` and `plan_budget_policy_update` to
`prodex-storage`. The command stores a tenant-scoped budget scope, a
`BudgetLimit`, and update time. PostgreSQL and SQLite now expose request-path
DML plans for upserting `prodex_budget_policies`; migration DDL remains in
explicit migrator plans.

Add `plan_application_budget_policy_lifecycle` to `prodex-application`. The
planner accepts a `BudgetUpdate` control-plane action, a matching
`BudgetPolicyUpdateCommand`, durable store selection, and audit digests. It
rejects wrong operations, wrong resource kinds, and cross-tenant action/policy
pairs before selecting storage. It always emits append-only audit storage and
emits budget policy storage only when the decision is authorized.

## Consequences

Control-plane budget updates become auditable and tenant-scoped before budget
limits are consumed by data-plane reservation logic. Client-visible errors do
not expose tenant IDs, budget scopes, or internal SQL details.
