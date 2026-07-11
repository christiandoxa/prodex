# ADR 0770: Redact reservation record debug output

## Status

Accepted

## Context

`ReservationRecord` carries tenant IDs, call IDs, reservation IDs, reserved
usage amounts, and recovery timestamps for abandoned-reservation handling.
Derived `Debug` exposed those values in diagnostics.

## Decision

Implement custom `Debug` for `ReservationRecord`. Preserve the record field
shape while redacting identifiers, reserved usage, and recovery timestamps.

## Consequences

Diagnostics still show that a reservation record was present. Tenant IDs, call
IDs, reservation IDs, usage amounts, and recovery timing values no longer appear
through reservation-record debug formatting.
