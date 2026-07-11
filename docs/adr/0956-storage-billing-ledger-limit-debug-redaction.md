# ADR 0956: Storage Billing Ledger Limit Debug Redaction

## Status

Accepted

## Context

Billing ledger page-limit validation errors can carry rejected numeric limits.
Display output is already generic, but derived debug output would expose the
rejected limit value in diagnostics.

## Decision

Use a custom `Debug` implementation for `BillingLedgerPageLimitError`. Redact
rejected limit values while preserving error variant names.

## Consequences

Storage diagnostics can distinguish billing ledger page-limit failures without
leaking rejected numeric limits.
