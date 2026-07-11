# ADR 0271: Data-Plane Authorization Boundary Resolver

## Status

Accepted

## Context

The data plane has two normal authorization surfaces: inference access through
virtual keys and quota/budget reads. The boundary enum already models these as
`DataPlaneInference` and `DataPlaneQuota`, but adapters still needed a
canonical way to map a domain `AuthorizationRequirement` to the correct
data-plane boundary.

Without a resolver, a control-plane credential or break-glass credential could
be accidentally routed through an ad hoc data-plane check if an adapter matched
only resource and action.

## Decision

Add `data_plane_boundary_for_requirement` to `prodex-authz`.

The resolver returns:

- `BoundaryKind::DataPlaneInference` only for `VirtualKey/Read`,
  `CredentialScope::DataPlane`, and `Role::Operator`;
- `BoundaryKind::DataPlaneQuota` only for `Budget/Read`,
  `CredentialScope::DataPlane`, and `Role::Operator`.

All control-plane, break-glass, wrong-action, and insufficient-role
requirements return `None`.

## Consequences

Data-plane adapters can select authorization boundaries from one canonical
function instead of duplicating requirement matching. Control-plane and
break-glass credentials remain unable to become inference or quota bypasses via
resource/action-only matching.
