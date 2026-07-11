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
the source of truth for that cost budget. PostgreSQL-backed admission now also
enforces cumulative per-key or grouped request budgets in the same reservation
transaction, while Redis enforces distributed RPM/TPM. Local request-count and
grouped-budget checks remain only for non-PostgreSQL compatibility backends.

## Consequences

Durable-backed request, token, and cost budgets no longer depend on stale
process-local usage in multi-replica deployments. Non-durable file/Redis
gateways and policy-only keys keep existing local budget behavior. Regression
coverage lives in the gateway key/budget tests and the real-PostgreSQL repository
and two-proxy proofs described by ADR 1066.
