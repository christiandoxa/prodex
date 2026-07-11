# ADR 0767: Redact reservation reconciliation error debug output

## Status

Accepted

## Context

`ReservationReconciliationError` carries usage amounts for post-provider
accounting decisions. Derived `Debug` exposed those amounts in diagnostics.

## Decision

Implement custom `Debug` for `ReservationReconciliationError`. Preserve the
reconciliation failure variant shape while redacting all usage amount fields.

## Consequences

Diagnostics still show which reconciliation invariant failed. Usage amounts no
longer appear through reconciliation-error debug formatting.
