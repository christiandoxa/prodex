# ADR 0966: Application Budget Policy Update Debug Redaction

## Status

Accepted

## Context

Application budget-policy update plans carry tenant-scoped budget policy
commands and nested storage plans. Derived debug output would expose tenant
identifiers, budget scopes, and policy values in diagnostics.

## Decision

Use custom `Debug` implementations for
`ApplicationBudgetPolicyUpdateRequest`, `ApplicationBudgetPolicyUpdatePlan`,
and `ApplicationBudgetPolicyUpdateError`. Redact budget policy commands,
storage plans, and nested storage errors while preserving planner and error
variant names.

## Consequences

Application diagnostics can distinguish budget-policy update planner shapes
without leaking tenant identifiers, budget scopes, or policy values.
