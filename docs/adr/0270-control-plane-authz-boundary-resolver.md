# ADR 0270: Control-Plane Authorization Boundary Resolver

## Status

Accepted

## Context

`prodex-control-plane` owns the canonical operation registry, while
`prodex-authz` owns reusable boundary authorization checks. After adding
explicit boundaries for control-plane lifecycle, secret, budget, billing, audit,
policy, and configuration operations, authz callers still needed a canonical
way to select the correct boundary from a resource/action requirement without
duplicating match logic.

Without a resolver, an adapter could continue to use a generic admin boundary
even when the operation registry defines a more precise resource/action pair.

## Decision

Add `control_plane_boundary_for_requirement` to `prodex-authz`. It accepts a
domain `AuthorizationRequirement` and returns an explicit control-plane
`BoundaryKind` only when the requirement matches a supported control-plane
resource, action, role, and `CredentialScope::ControlPlane`.

The resolver intentionally returns `None` for generic admin update checks,
data-plane requirements, and break-glass requirements. Those paths must remain
separate from normal control-plane operation authorization.

Regression tests cover every explicit control-plane authz boundary currently
owned by `prodex-authz` and require each resource/action/role tuple to resolve
back to the same boundary. The crate intentionally does not depend on
`prodex-control-plane`, preserving the existing dependency guard.

## Consequences

Composition roots and HTTP adapters can use one authz resolver instead of
duplicating resource/action matching. Future control-plane operations should
add or intentionally reject an authz boundary before adapters wire them through
normal control-plane authorization.
