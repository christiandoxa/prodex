# ADR 0880: Domain accounting display redaction

## Status

Accepted.

## Context

Reservation commit mismatch and reservation reconciliation response planners
already expose stable redacted messages. Their raw `Display` strings still
named tenant, call, reservation, amount, underflow, and overflow mismatch
classes. Those classes are useful as typed variants, but raw display text can be
accidentally surfaced by API boundaries.

## Decision

Render all `ReservationCommitMismatch` variants as `reservation commit rejected`
and all `ReservationReconciliationError` variants as
`reservation reconciliation rejected`. Keep the existing response planners and
codes unchanged so callers can still classify failures by matching typed
variants.

Regression coverage pins the generic display strings and keeps existing debug
redaction checks for tenant IDs, call IDs, reservation IDs, and usage amounts.

## Consequences

Operational code should use structured variants for audit and metrics
classification. Raw accounting display output stays low-cardinality and safe
for accidental exposure.
