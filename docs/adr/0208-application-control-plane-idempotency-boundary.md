# 0208: Application Control Plane Idempotency Boundary

## Status

Accepted

## Context

The control-plane domain marks mutating operations as requiring idempotency and
can derive a tenant-scoped `IdempotentOperation`. Composition roots still need a
side-effect-free application preflight that rejects mutating admin actions
without a replay key before authorization, audit storage, or mutation storage is
adapted.

Without this boundary, retryable destructive operations such as audit retention
purge can be wired without the replay/conflict guard expected by enterprise API
governance.

## Decision

Add `plan_application_control_plane_idempotency`.

The planner accepts a control-plane action, optional validated
`IdempotencyKey`, and request fingerprint. Mutating actions require a key and
produce the tenant-scoped idempotent operation used by replay storage.
Read-only actions produce no idempotency operation. Missing mutating keys map to
a stable redacted application error response.

## Consequences

- Composition roots have one preflight for mutating admin idempotency.
- Audit retention purge cannot proceed as a retryable mutation without a key.
- Read-only control-plane operations remain lightweight.
- Tenant IDs, keys, and fingerprints stay out of client-visible failures.
