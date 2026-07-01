# ADR 0943: Storage Expired Reservation Error Debug Redaction

## Status

Accepted

## Context

Expired reservation recovery planner errors can carry tenant identifiers when a
storage key does not match the reservation record tenant. Display output is
generic, but derived debug output would expose the mismatched tenant values.

## Decision

Use a custom `Debug` implementation for `ExpiredReservationRecoveryPlanError`.
Redact tenant identifiers while preserving error variant names.

## Consequences

Storage diagnostics can distinguish expired-reservation recovery failures
without leaking tenant identifiers from mismatch errors.
