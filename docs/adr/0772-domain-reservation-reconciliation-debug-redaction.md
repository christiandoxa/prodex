# ADR 0772: Redact reservation reconciliation debug output

## Status

Accepted

## Context

`ReservationReconciliation` carries a redacted commit plus committed and
released ledger events after provider completion or interruption. Derived
`Debug` exposed nested tenant IDs, call IDs, reservation IDs, and usage amounts.

## Decision

Implement custom `Debug` for `ReservationReconciliation`. Preserve the
reconciliation reason and event-presence shape while redacting nested commit and
ledger event details.

## Consequences

Diagnostics still show why reconciliation ran and whether a release event was
present. Nested accounting identifiers and usage amounts no longer appear
through reconciliation debug formatting.
