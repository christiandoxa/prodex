# ADR 0249: Control-Plane SCIM User Lifecycle

## Status

Accepted

## Context

The enterprise target architecture requires SCIM in the control plane for user
lifecycle management. The existing control-plane boundary already had a user
invite operation, but SCIM create, update, and delete were not explicit
operations. Leaving SCIM as an adapter-specific alias would make authorization,
idempotency, and audit coverage easier to bypass or forget.

## Decision

Add explicit `ControlPlaneOperation` variants for SCIM user create, update, and
delete. Each operation targets `ResourceKind::User`, requires `Role::Admin`,
uses the matching lifecycle `ResourceAction`, requires idempotency, and emits a
distinct immutable audit action:

- `control_plane.scim_user.create`
- `control_plane.scim_user.update`
- `control_plane.scim_user.delete`

Keep these operations in `ControlPlaneOperation::ALL` so lifecycle, audit, and
resource-kind mismatch tests remain exhaustive.

## Consequences

SCIM HTTP adapters can stay thin: they must map protocol requests into these
control-plane operations rather than defining authorization behavior locally.
SCIM delete is treated as a security-sensitive admin mutation and must use the
same tenant-scoped idempotency and append-only audit contracts as other
control-plane mutations.
