# ADR 0116: Application recovery lease orchestration

## Status

Accepted

## Context

Expired reservation recovery now has durable PostgreSQL/SQLite plans and Redis
lease primitives. Composition roots still need one side-effect-free application
use case that selects both the durable recovery work and optional multi-replica
coordination without duplicating tenant checks in legacy adapters.

## Decision

Extend `plan_application_expired_reservation_recovery` with optional
`ApplicationExpiredReservationRecoveryCoordinationRequest`. When supplied, the
application boundary plans a tenant-scoped Redis recovery lease using the same
tenant context and storage key as the durable recovery command.

The application plan now returns:

- the gateway recovery plan with trace propagation;
- the durable PostgreSQL or SQLite recovery SQL plan; and
- an optional Redis lease acquisition plan for the recovery shard.

Invalid Redis coordination input is surfaced as an application-level recovery
planning error before any adapter executes work.

## Consequences

Multi-replica recovery workers can be wired through one boundary contract:
Redis coordinates shards, while the durable store remains the source of truth
for counters and ledger events. The crate remains free of Redis drivers,
async runtimes, HTTP frameworks, and filesystem access.
