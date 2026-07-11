# ADR 0925: Storage billing ledger query debug redaction

## Status

Accepted.

## Context

Billing ledger query commands and plans carry tenant-scoped storage keys,
tenant IDs, time ranges, and page limits. Those fields are needed by storage
adapters but should not appear in generic debug output.

## Decision

Use custom `Debug` implementations for `BillingLedgerQueryCommand` and
`BillingLedgerQueryPlan`. Redact storage key, tenant ID, time-range, and page
limit details while preserving the low-cardinality sort order.

Regression coverage rejects tenant ID, timestamp, and page-limit values in
rendered billing ledger query debug output.

## Consequences

Storage diagnostics can identify query shape and sort direction without
exposing tenant or pagination details. Billing query planning behavior is
unchanged.
