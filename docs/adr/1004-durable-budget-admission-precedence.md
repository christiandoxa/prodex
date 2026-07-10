# ADR 1004: Durable budget admission precedence

## Status

Accepted.

## Context

Phase 0 accounting requirements call out local in-memory admission before
durable persistence as a multi-replica risk. Admin-managed virtual keys backed
by SQLite or PostgreSQL already reserve token/cost budget in durable storage,
but the gateway still rejected a request first when stale local spend indicated
the key budget was exhausted.

## Decision

For admin-managed virtual keys whose tenant and virtual-key identifiers allow a
durable SQLite/PostgreSQL reservation, local admission no longer uses
`spend_microusd` to enforce the per-key cost budget. Durable reservation remains
the source of truth for that cost budget. Local request-count, RPM, TPM, model,
and grouped-budget checks are preserved because they are not yet fully covered
by the durable reservation path.

## Consequences

Durable-backed key cost budgets are less likely to fail closed from stale
process-local spend state in multi-replica deployments. Non-durable file/Redis
gateways and policy-only keys keep existing local budget behavior. Remaining
work: move request-count, RPM/TPM, and grouped budget enforcement into durable
or Redis-backed atomic operations before declaring multi-replica accounting
complete. Regression coverage lives in
`crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gateway_keys.rs`.
