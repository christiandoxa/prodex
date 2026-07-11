# 0632: Domain Reservation Record Zero Reserved Guard

## Status

Accepted

## Context

Budget admission rejects zero reservation estimates, but
`ReservationRecord::from_request` could still be called directly with a
zero-usage request. That produced abandoned-reservation records whose recovery
path had no meaningful balance to release.

## Decision

`ReservationRecord::from_request` now rejects zero reserved usage with
`ReservationRecoveryError::ZeroReserved`. The stable recovery error planner maps
it to `reservation_reserved_usage_invalid`.

## Consequences

- Recovery records must carry non-zero reserved usage.
- Direct callers cannot bypass the admission zero-estimate guard.
- Client-visible recovery errors remain stable and redacted.
