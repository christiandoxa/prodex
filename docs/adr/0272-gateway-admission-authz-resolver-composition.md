# ADR 0272: Gateway Admission Authorization Resolver Composition

## Status

Accepted

## Context

Gateway admission authorizes inference requests before budget reservation and
provider invocation. After adding `data_plane_boundary_for_requirement` to
`prodex-authz`, gateway admission still selected `DataPlaneInference` directly.
That was behaviorally correct, but it did not prove that admission was using the
canonical data-plane requirement resolver.

## Decision

`prodex-gateway-core` now resolves the inference authorization boundary from a
domain `AuthorizationRequirement` for `VirtualKey/Read`,
`CredentialScope::DataPlane`, and `Role::Operator`.

The resulting `BoundaryKind` is retained in `GatewayAdmissionPlan` so adapters
and tests can observe which authz boundary was used. If the resolver ever fails
to resolve the built-in inference requirement, gateway admission returns a
stable redacted `gateway_authorization_unavailable` response instead of
panicking or falling back to generic authorization.

## Consequences

Data-plane admission stays aligned with the authz resolver and continues to
reject control-plane or break-glass credentials before reservation and provider
invocation. The plan exposes boundary evidence without importing HTTP
frameworks, storage drivers, or provider SDKs into gateway core.
