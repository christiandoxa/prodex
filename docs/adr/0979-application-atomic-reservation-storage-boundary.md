# ADR 0979: Application atomic reservation storage boundary

## Status

Accepted

## Context

`prodex-gateway-core` already plans data-plane admission around an
`AtomicReservationCommand`, and the storage adapter crates already expose
PostgreSQL and SQLite atomic-reservation SQL plans. The remaining legacy gateway
path in `prodex-app` still keeps durable-store branching in composition code
instead of going through one application boundary before the durable reservation
backend is fully wired.

Without an application-level selector, the migration away from local
compatibility admission would keep leaking storage-adapter choices into
`prodex-app`, which is the dependency direction this refactor is trying to
remove.

## Decision

Add `plan_application_atomic_reservation` to `prodex-application`. The boundary
accepts a durable store plus an `AtomicReservationCommand`, then selects the
PostgreSQL or SQLite atomic-reservation SQL plan and maps storage-planner
failures through one stable application error response:

- `atomic_reservation_storage_unavailable`

The boundary remains side-effect-free. It does not execute SQL or replace the
legacy gateway request path by itself; it only gives composition roots one
place to choose durable reservation DML before runtime wiring moves off the
local compatibility path.

## Consequences

The repository now has an application-layer durable reservation selector that
matches the existing usage-reconciliation and expired-recovery boundaries.
`prodex-app` can migrate the legacy request path incrementally without adding
new storage-adapter branching to the composition root. The remaining work is to
execute this boundary from the request-serving gateway path instead of mutating
local usage state.
