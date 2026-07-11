# ADR 0929: Storage expired-reservation recovery debug redaction

## Status

Accepted.

## Context

Expired-reservation recovery commands and plans carry storage keys, budget
snapshots, reservation records, recovery timestamps, and ledger events. These
values are needed by storage adapters but should not appear in generic debug
output.

## Decision

Use custom `Debug` implementations for `ExpiredReservationRecoveryCommand` and
`ExpiredReservationRecoveryPlan`. Redact storage keys, budget/accounting
payloads, reservation records, recovery timestamps, and ledger events.

Regression coverage rejects tenant ID, call ID, reservation ID, timestamp, and
usage amount values in rendered expired-reservation recovery debug output.

## Consequences

Storage diagnostics can identify recovery command/plan shape without exposing
tenant or accounting values. Recovery behavior is unchanged.
