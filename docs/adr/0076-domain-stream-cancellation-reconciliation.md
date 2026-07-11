# ADR 0076: Domain stream cancellation reconciliation

## Status
Accepted

## Context
Prodex gateway streams can be cancelled by the client or interrupted by the
transport after some usage has already happened upstream. Enterprise billing
must reconcile the reservation exactly once: commit measured partial usage and
release any unused reservation without dropping ledger evidence.

## Decision
`prodex-domain` now exposes reservation reconciliation primitives for completed,
cancelled, and stream-interrupted calls. Reconciliation is based on the durable
`ReservationRecord` and the measured actual usage. It produces:

- an updated budget snapshot;
- the reservation commit that charges actual usage;
- an append-only committed ledger event; and
- an optional released ledger event for the unused reservation delta.

Actual usage greater than the reservation is rejected by the domain primitive so
storage adapters and gateway code must handle overshoot deliberately instead of
silently charging beyond the reserved amount.

Committed usage overflow is also rejected before the budget snapshot is updated,
so reconciliation never silently saturates tenant usage totals.

## Consequences
Gateway/storage implementations can perform cancellation reconciliation inside a
single durable transaction keyed by tenant, call, and reservation identifiers.
This keeps cancellation semantics deterministic across replicas and prevents
both duplicate charging and abandoned reservations while the domain crate stays
independent from HTTP, database, and provider dependencies.
