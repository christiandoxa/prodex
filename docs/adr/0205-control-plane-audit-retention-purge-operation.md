# 0205: Control Plane Audit Retention Purge Operation

## Status

Accepted

## Context

Audit retention purge now has domain, storage, SQL adapter, and application
planning boundaries. The control plane still needs an explicit operation so
purge execution is authorized and audited as a security-sensitive mutation
instead of being treated as a generic audit export or background storage call.

Enterprise security requirements demand authorization on every resource/action
and immutable audit events for security-sensitive actions.

## Decision

Add `ControlPlaneOperation::AuditRetentionPurge`.

The operation targets `ResourceKind::AuditLog`, requires `ResourceAction::Delete`
with Admin role, and emits the audit action
`control_plane.audit.retention_purge`. Viewer and data-plane credentials are
denied through the existing control-plane authorization path and still produce
denial audit events.

## Consequences

- Audit retention deletes have an explicit control-plane authorization surface.
- Retention purge cannot reuse read/export permissions.
- Purge attempts produce immutable audit events with a stable action string.
- Existing credential-scope, role, and tenant checks remain shared.
