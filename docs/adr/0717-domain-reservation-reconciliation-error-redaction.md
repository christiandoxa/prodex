# ADR 0717: Redact Reservation Reconciliation Error Details

## Status

Accepted

## Context

Reservation reconciliation runs after provider responses and streaming
interruption paths. Its error variants keep structured usage amounts so trusted
callers can classify underflow, actual-over-reserved, and overflow failures.
The `Display` strings still echoed those amounts.

## Decision

Keep the structured `ReservationReconciliationError` fields, but make its
`Display` strings generic. Stable response planners continue returning fixed
accounting error envelopes.

## Consequences

- Usage amounts are not echoed through reconciliation error strings.
- Internal code can still match variants and inspect structured fields when
  trusted diagnostics need them.
- Post-provider accounting errors stay aligned with regulated error-redaction
  requirements.
