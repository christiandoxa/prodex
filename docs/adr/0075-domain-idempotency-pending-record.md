# ADR 0075: Domain idempotency pending records

## Status
Accepted

## Context
Enterprise data-plane and control-plane retries must not execute the same
mutation twice. Completed-response replay is not sufficient for multi-replica
operation because a duplicate request can arrive while the first replica is
still reserving budget, writing a ledger event, or mutating a control-plane
resource.

## Decision
`prodex-domain` now models idempotency storage as either:

- `Pending`, recorded before a mutation is executed; or
- `Completed`, recorded with the durable response that can be replayed.

The replay decision is tenant-, key-, and request-fingerprint-aware. A duplicate
request that matches a pending record receives an explicit in-progress decision
instead of executing again. A completed record replays the stored response. Any
cross-tenant, wrong-key, or different-fingerprint reuse is a conflict.

## Consequences
Storage adapters can implement this with database uniqueness on
`(tenant_id, idempotency_key)` and a transaction that inserts `Pending` before
side effects. This keeps retry behavior deterministic across replicas and
prevents duplicate billing/control-plane mutations without coupling the domain
crate to a database or HTTP framework.
