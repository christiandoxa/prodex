# ADR 0905: Storage billing ledger limit display redaction

## Status

Accepted.

## Context

Billing ledger page limits are request-controlled accounting query inputs. ADR
0715 removed rejected numeric values from `Display`, but raw display output
still distinguished zero values from oversized values.

## Decision

Render `BillingLedgerPageLimitError` with one stable message:
`billing ledger page limit is invalid`. Keep typed variants unchanged for
trusted classification.

Regression coverage pins exact display strings for zero and oversized limits.

## Consequences

Storage query boundaries can safely fall back to raw display strings without
exposing page-limit validation shape. Diagnostics should continue matching
typed variants for exact reasons.
