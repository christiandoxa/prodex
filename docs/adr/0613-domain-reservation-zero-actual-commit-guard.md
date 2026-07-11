# ADR 0613: Domain Reservation Zero Actual Commit Guard

## Status

Accepted.

## Context

Budget reservation rejects zero estimates, but direct reservation commit could
still accept zero actual usage. That allowed a no-op commit path to release
reserved balance without recording any committed usage.

## Decision

`commit_reservation` now rejects `UsageAmount::ZERO` actual usage with
`reservation_actual_usage_invalid`.

## Consequences

Direct commit callers must either commit non-zero actual usage or use recovery
or reconciliation flows for releases. API-facing errors stay stable and
redacted.
