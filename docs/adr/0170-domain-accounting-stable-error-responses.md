# 0170: Domain Accounting Stable Error Responses

## Status

Accepted

## Context

Enterprise accounting is reservation-based: requests reserve estimated usage
before provider dispatch, commit actual usage afterward, release unused
capacity, and recover abandoned reservations. Raw accounting errors include
tenant IDs, call IDs, reservation IDs, requested/available usage, reserved and
actual amounts, and arithmetic details.

Those fields are required for durable storage plans, append-only ledgers, and
trusted diagnostics. They must not be returned directly through public gateway
or control-plane API responses.

## Decision

`prodex-domain` owns stable accounting response planners:

- `plan_budget_rejection_response`
- `plan_reservation_commit_mismatch_response`
- `plan_reservation_recovery_error_response`
- `plan_reservation_reconciliation_error_response`

The planners map domain accounting failures to stable status/code/message
response plans. They deliberately omit tenant IDs, call IDs, reservation IDs,
requested/available usage, reserved/actual amounts, ledger keys, and arithmetic
details.

## Consequences

- Data-plane admission can reject over-budget requests before upstream dispatch
  without leaking tenant accounting internals.
- Post-provider reconciliation and recovery paths can fail closed with stable
  redacted responses.
- Raw accounting errors remain available for trusted audit and diagnostics.
