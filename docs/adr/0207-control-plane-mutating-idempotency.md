# 0207: Control Plane Mutating Idempotency

## Status

Accepted

## Context

Enterprise API governance requires idempotency keys for mutating admin
operations. Prodex already has reusable domain idempotency primitives, but
control-plane operations need an explicit boundary that says which operations
are mutating and how to build the tenant-scoped idempotent operation used by
replay storage.

Audit retention purge is a destructive control-plane mutation, so retrying it
must not bypass replay/conflict checks or run as a plain audit export.

## Decision

Add `ControlPlaneOperation::requires_idempotency` and
`ControlPlaneActionRequest::idempotent_operation`.

Mutating operations, including `AuditRetentionPurge`, require idempotency.
Read-only operations such as billing read and audit export do not. The
idempotent operation is tenant-scoped from the requested resource tenant and
carries the validated `IdempotencyKey` plus request fingerprint for replay
conflict detection.

## Consequences

- Composition roots can enforce idempotency for mutating control-plane actions.
- Audit retention purge retries use the same domain replay/conflict primitives.
- Read-only operations do not create unnecessary replay records.
- Raw idempotency keys remain behind existing idempotency validation planners.
