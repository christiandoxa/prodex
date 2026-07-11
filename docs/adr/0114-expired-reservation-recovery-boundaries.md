# ADR 0114: Expired reservation recovery boundary orchestration

## Status

Accepted

## Context

The storage layer can recover abandoned reservations, but enterprise gateway
composition roots must not call storage adapters directly with duplicated
business checks. Recovery cleanup needs the same tenant validation, trace
propagation, and durable-store selection pattern as post-provider usage
reconciliation.

## Decision

Add a side-effect-free expired reservation recovery use case to
`prodex-gateway-core` and `prodex-application`.

`prodex-gateway-core` now exposes `plan_gateway_expired_reservation_recovery`,
which validates that the tenant context, storage key, and reservation record
belong to the same tenant, delegates release semantics to the storage-domain
contract, and emits a propagated gateway span named
`prodex.gateway.expired_reservation_recovery`.

`prodex-application` now exposes
`plan_application_expired_reservation_recovery`, which lets composition roots
select PostgreSQL or SQLite recovery SQL plans without reimplementing accounting
rules in legacy adapters.

## Consequences

Expired reservation cleanup can be wired from schedulers or control loops while
keeping tenant isolation, accounting semantics, and trace propagation in the
same boundary crates used by the request lifecycle. The implementation remains
side-effect-free until adapter crates execute the selected durable-store plan.
