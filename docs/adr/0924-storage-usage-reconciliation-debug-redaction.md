# ADR 0924: Storage usage reconciliation debug redaction

## Status

Accepted.

## Context

Usage reconciliation commands and plans carry storage keys, budget snapshots,
reservation records, actual usage, reconciliation output, and ledger events.
Those values are needed by storage adapters but are too detailed for generic
debug output.

## Decision

Use custom `Debug` implementations for `UsageReconciliationCommand` and
`UsageReconciliationPlan`. Redact storage keys, budget/accounting payloads,
reservation records, actual usage, reconciliation details, and ledger events
while keeping the reconciliation reason visible as low-cardinality shape.

Regression coverage rejects tenant ID, call ID, reservation ID, and usage
amounts in rendered usage reconciliation debug output.

## Consequences

Storage diagnostics can identify usage reconciliation command/plan shape without
exposing tenant or accounting values. Reconciliation behavior is unchanged.
