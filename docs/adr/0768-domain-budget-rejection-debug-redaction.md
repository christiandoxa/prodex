# ADR 0768: Redact budget rejection debug output

## Status

Accepted

## Context

`BudgetRejection` carries requested and available usage amounts for pre-upstream
reservation decisions. Derived `Debug` exposed those amounts in diagnostics.

## Decision

Implement custom `Debug` for `BudgetRejection`. Preserve the rejection reason
while redacting requested and available usage amounts.

## Consequences

Diagnostics still show why reservation was rejected. Requested and available
usage amounts no longer appear through budget-rejection debug formatting.
