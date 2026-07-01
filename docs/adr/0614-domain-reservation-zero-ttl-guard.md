# ADR 0614: Domain Reservation Zero TTL Guard

## Status

Accepted.

## Context

Reservation records derive their expiry from `created_at_unix_ms + ttl_ms`.
Accepting a zero TTL creates an immediately expired reservation, which can make
reserve/recover behavior indistinguishable from stale recovery.

## Decision

`ReservationRecord::from_request` now rejects `ttl_ms == 0` with
`reservation_ttl_invalid`.

## Consequences

Every reservation record must have a non-zero recovery window. Callers must
provide an explicit positive TTL before a reservation can be persisted.
