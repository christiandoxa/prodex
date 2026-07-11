# ADR 0775: Redact budget snapshot debug output

## Status

Accepted

## Context

`BudgetSnapshot` carries reserved and committed usage totals for tenant
accounting decisions. Even with `UsageAmount` redaction, derived `Debug` would
automatically expose future fields if they are added.

## Decision

Implement custom `Debug` for `BudgetSnapshot`. Preserve the snapshot field
shape while redacting reserved and committed usage totals.

## Consequences

Diagnostics still show that budget state was present. Reserved and committed
usage totals no longer appear through budget-snapshot debug formatting.
