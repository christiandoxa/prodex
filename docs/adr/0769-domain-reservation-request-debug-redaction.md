# ADR 0769: Redact reservation request debug output

## Status

Accepted

## Context

`ReservationRequest` carries tenant IDs, call IDs, reservation IDs, and
estimated usage before upstream dispatch. Derived `Debug` exposed those values
in diagnostics.

## Decision

Implement custom `Debug` for `ReservationRequest`. Preserve the request field
shape while redacting all identifier and usage estimate values.

## Consequences

Diagnostics still show that a reservation request was present. Tenant IDs, call
IDs, reservation IDs, and usage estimates no longer appear through request
debug formatting.
