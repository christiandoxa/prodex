# ADR 0942: Storage Billing Ledger Error Debug Redaction

## Status

Accepted

## Context

Billing ledger query planner errors can carry tenant identifiers and rejected
time ranges. Display output is already generic, but derived debug output would
expose those values in diagnostics.

## Decision

Use a custom `Debug` implementation for `BillingLedgerQueryPlanError`. Redact
tenant identifiers and range timestamps while preserving error variant names.

## Consequences

Storage diagnostics can distinguish billing ledger query error variants without
leaking tenant topology or rejected time ranges.
