# ADR 0766: Redact reservation recovery error debug output

## Status

Accepted

## Context

`ReservationRecoveryError` carries tenant IDs and usage amounts when abandoned
reservation recovery fails. Derived `Debug` exposed those values in diagnostics.

## Decision

Implement custom `Debug` for `ReservationRecoveryError`. Preserve the recovery
failure variant shape while redacting tenant IDs and usage amounts.

## Consequences

Diagnostics still show which recovery invariant failed. Tenant identifiers and
usage amounts no longer appear through recovery-error debug formatting.
