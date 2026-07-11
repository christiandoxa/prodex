# ADR 0267: Control-Plane Audit Authorization Boundaries

## Status

Accepted

## Context

Audit export and audit retention purge are distinct regulated control-plane
operations. Export reads audit records out of the system, while retention purge
deletes expired audit records after domain retention and legal-hold checks.
Both require precise resource/action authorization and must not be reachable
with data-plane credentials.

`ControlPlaneAuditExport` already modeled export, but retention purge could
still fall back to broad admin authorization in adapters that use
`prodex-authz` directly.

## Decision

Keep `BoundaryKind::ControlPlaneAuditExport` as the explicit
`AuditLog/Export` admin boundary and add
`BoundaryKind::ControlPlaneAuditRetentionPurge` as the explicit
`AuditLog/Delete` admin boundary.

Both boundaries require:

- `CredentialScope::ControlPlane`;
- `Role::Admin`;
- tenant access through `authorize_boundary_resource`.

Data-plane and break-glass scopes are rejected by these normal audit operation
boundaries. Break-glass access remains separate and audited.

## Consequences

Audit adapters can authorize export and retention purge against the same
resource/action pairs used by `ControlPlaneOperation::AuditExport` and
`ControlPlaneOperation::AuditRetentionPurge`. Stable redacted authorization
responses remain unchanged.
