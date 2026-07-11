# ADR 0012: Admin mutation idempotency key

## Status

Accepted.

## Context

The enterprise control-plane target requires idempotency keys for mutating admin
operations so retries do not create duplicate resources or apply duplicate side
effects. Existing CLI and dashboard workflows did not send an idempotency key, so
a hard requirement would be a compatibility break.

## Decision

Admin mutations accept an optional `Idempotency-Key` header. When present, the
runtime gateway records the key scoped to admin principal/governance scope, HTTP
method, and resource path before the mutation is dispatched. A duplicate key for
the same admin scope and method/resource is rejected with a stable
`409 duplicate_idempotency_key` JSON error before the second mutation runs.

This is a Phase 0 compatibility-preserving guard. Existing callers without the
header continue to work. A future durable control plane should persist
idempotency records across replicas and return the original mutation result for
exact replays.

## Consequences

- Retried admin mutations can opt in to duplicate-side-effect protection now.
- Two scoped admins can reuse the same client-generated idempotency key without
  causing cross-tenant denial.
- The current in-process cache is not sufficient for multi-replica durability;
  production control-plane storage must move idempotency records into the shared
  database.
- Failed first attempts may reserve an idempotency key until process restart;
  this is acceptable for the incremental hardening phase and is documented as a
  remaining risk.
