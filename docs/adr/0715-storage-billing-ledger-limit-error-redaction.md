# ADR 0715: Redact Billing Ledger Limit Validation Errors

## Status

Accepted

## Context

Billing ledger reads are tenant-owned accounting queries. The storage boundary
already returns stable error envelopes and avoids echoing tenant IDs, usage
amounts, backend names, and time range inputs. Oversized billing ledger page
limits still appeared in the `Display` string for the page-limit validation
error.

## Decision

Keep the structured `BillingLedgerPageLimitError::TooLarge` variant, but make
its `Display` text generic. Callers can continue matching the variant while
Prodex-owned API layers expose the stable storage error envelope.

## Consequences

- Rejected billing ledger page-limit values are not echoed through storage
  planner error strings.
- The page-limit maximum remains available as typed code, not as user-provided
  request echo.
- Billing ledger validation stays aligned with regulated error-redaction
  requirements.
