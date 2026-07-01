# ADR 0771: Redact reservation commit debug output

## Status

Accepted

## Context

`ReservationCommit` carries tenant IDs, call IDs, reservation IDs, reserved
usage, and actual usage for accounting commit validation. Derived `Debug`
exposed those values in diagnostics.

## Decision

Implement custom `Debug` for `ReservationCommit`. Preserve the commit field
shape while redacting identifiers and usage amounts.

## Consequences

Diagnostics still show that a reservation commit was present. Tenant IDs, call
IDs, reservation IDs, and usage amounts no longer appear through commit debug
formatting.
