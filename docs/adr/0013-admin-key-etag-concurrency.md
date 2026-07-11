# ADR 0013: Admin key ETag concurrency control

## Status

Accepted.

## Context

The enterprise control-plane target requires concurrency control using ETag or
resource versions where relevant. Gateway virtual-key admin operations currently
allow last-writer-wins updates and deletes, which can hide concurrent dashboard,
CLI, or automation changes.

## Decision

Single-key `GET /v1/prodex/gateway/keys/{name}` returns an `ETag` header derived
from the key resource update timestamp. `PATCH` and `DELETE` on the same resource
accept optional `If-Match`. When supplied, a stale value is rejected with stable
JSON error `412 precondition_failed` before the mutation is dispatched. Existing
clients that do not send `If-Match` remain compatible.

## Consequences

- Automation can opt in to optimistic concurrency protection now.
- Existing CLI/dashboard behavior remains backward compatible.
- Timestamp-derived ETags are a Phase 0 compatibility step; a future durable
  control plane should use explicit resource-version columns that are monotonic
  across replicas and storage backends.
