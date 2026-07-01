# ADR 0776: Redact budget limit debug output

## Status

Accepted

## Context

`BudgetLimit` carries tenant accounting thresholds used before provider
dispatch. Derived `Debug` exposed raw token and cost ceilings in diagnostics.

## Decision

Implement custom `Debug` for `BudgetLimit`. Preserve the limit field shape while
redacting the maximum usage amount.

## Consequences

Diagnostics still show that a budget limit was present. Raw token and cost
ceilings no longer appear through budget-limit debug formatting.
