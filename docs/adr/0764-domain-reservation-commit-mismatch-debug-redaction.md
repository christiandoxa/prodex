# ADR 0764: Redact reservation commit mismatch debug output

## Status

Accepted

## Context

`ReservationCommitMismatch` carries tenant IDs, call IDs, reservation IDs, and
usage amounts for trusted accounting decisions. Derived `Debug` exposed those
values in diagnostics.

## Decision

Implement custom `Debug` for `ReservationCommitMismatch`. Preserve the mismatch
variant shape while redacting all expected and actual values.

## Consequences

Diagnostics still show which commit invariant failed. Tenant IDs, call IDs,
reservation IDs, and usage amounts no longer appear through mismatch debug
formatting.
