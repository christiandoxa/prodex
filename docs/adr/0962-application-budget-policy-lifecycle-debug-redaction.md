# ADR 0962: Application Budget Policy Lifecycle Debug Redaction

## Status

Accepted

## Context

Application budget-policy lifecycle plans carry control-plane action details,
budget policy command metadata, audit digest state, authorized/denied
decisions, and nested storage planning. Derived debug output would expose
tenant, budget-scope, and policy topology in diagnostics.

## Decision

Use custom `Debug` implementations for
`ApplicationBudgetPolicyLifecycleRequest`,
`ApplicationBudgetPolicyLifecyclePlan`, and
`ApplicationBudgetPolicyLifecycleError`. Redact control-plane action details,
budget policy command metadata, audit digests, decisions, storage plans,
route/resource mismatches, tenant mismatches, and nested update errors while
preserving planner and error variant names.

## Consequences

Application diagnostics can distinguish budget-policy lifecycle planner shapes
without leaking tenant identifiers, budget scopes, audit digests, or policy
command details.
