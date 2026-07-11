# ADR 0266: Control-Plane Publish Authorization Boundaries

## Status

Accepted

## Context

Policy and configuration publication are separate control-plane operations with
different resource kinds. The generic `ControlPlaneAdmin` authorization boundary
uses `Configuration/Update`, which is useful as a broad admin guard but too
coarse for publish adapters that must authorize every resource and action
explicitly.

Without dedicated publish boundaries, HTTP or control-plane adapters could
accidentally treat policy publication as generic configuration mutation, making
authorization logs and negative tests less precise.

## Decision

Add two explicit `prodex-authz` boundaries:

- `BoundaryKind::ControlPlanePolicyPublish` requires
  `CredentialScope::ControlPlane`, `Role::Admin`, `ResourceKind::Policy`, and
  `ResourceAction::PublishRevision`.
- `BoundaryKind::ControlPlaneConfigurationPublish` requires
  `CredentialScope::ControlPlane`, `Role::Admin`,
  `ResourceKind::Configuration`, and `ResourceAction::PublishRevision`.

Data-plane and break-glass scopes do not satisfy these normal control-plane
publication boundaries. Break-glass access continues to use its separate
audited boundary.

## Consequences

Policy and configuration publication adapters can authorize against the same
resource/action pairs used by `ControlPlaneOperation::PolicyPublish` and
`ControlPlaneOperation::ConfigurationPublish`. Stable redacted authorization
responses remain unchanged.
