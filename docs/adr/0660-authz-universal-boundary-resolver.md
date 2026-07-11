# 0660. Authz Universal Boundary Resolver

## Status

Accepted

## Context

`prodex-authz` had separate resolvers for data-plane and normal control-plane
requirements. Break-glass authorization was explicit at the authorizer, but a
caller that received a generic `AuthorizationRequirement` could not resolve the
break-glass requirement through the same fail-closed boundary selection path.

That can push adapters toward hand-written match logic or generic admin
fallbacks.

## Decision

Add `boundary_for_requirement` to `prodex-authz`. It composes the existing
data-plane and control-plane resolvers with a small break-glass resolver.

The normal `control_plane_boundary_for_requirement` behavior is unchanged:
break-glass requirements stay separate from normal control-plane operations.

## Consequences

- Generic adapters can resolve supported requirements to explicit authz
  boundaries without duplicating match logic.
- Break-glass still requires the dedicated `BreakGlassAdmin` boundary,
  `CredentialScope::BreakGlass`, `Role::Admin`, and `PrincipalKind::BreakGlass`.
- Unsupported resource/action/scope/role combinations continue to fail closed.
