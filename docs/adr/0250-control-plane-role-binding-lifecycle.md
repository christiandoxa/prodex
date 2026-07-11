# ADR 0250: Control-Plane Role-Binding Lifecycle

## Status

Accepted

## Context

The enterprise control plane must own role and permission management. If role
assignment is implemented as adapter-local user metadata, horizontal privilege
escalation and missing audit coverage become easy to introduce when adding SCIM
or admin APIs.

## Decision

Represent role assignment as a tenant-owned `ResourceKind::RoleBinding` and add
explicit control-plane operations for granting and revoking role bindings:

- `ControlPlaneOperation::RoleBindingGrant`
- `ControlPlaneOperation::RoleBindingRevoke`

Both operations require `CredentialScope::ControlPlane`, `Role::Admin`, tenant
authorization on the role-binding resource, tenant-scoped idempotency, and
append-only immutable audit actions:

- `control_plane.role_binding.grant`
- `control_plane.role_binding.revoke`

## Consequences

Control-plane adapters must route role changes through these operations instead
of treating role changes as ordinary user updates. Viewer and operator
principals cannot grant or revoke roles, and denial paths remain covered by the
same immutable audit contract as other security-sensitive admin mutations.
