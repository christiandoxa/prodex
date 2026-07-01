# ADR 0714: Redact Billing Ledger Range Validation Errors

## Status

Accepted

## Context

Billing ledger queries are tenant-owned accounting reads. Their request
validation can reject an inverted time range before any storage adapter runs.
Other storage planner errors already avoid echoing tenant IDs, usage amounts,
and backend details in `Display` output and stable error response plans.

## Decision

Keep `BillingLedgerQueryPlanError::StartAfterEnd` structured for callers, but
make its `Display` string generic. The stable response envelope remains
`billing_ledger_query_rejected` with a fixed message.

## Consequences

- Query timestamps are not echoed through storage planner error strings.
- Callers can still inspect `start_unix_ms` and `end_unix_ms` from the enum when
  they need structured diagnostics.
- Billing ledger error handling stays aligned with the redacted storage planner
  boundary.
