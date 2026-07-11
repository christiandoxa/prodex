# ADR 0273: Gateway Quota Read Authorization Plan

## Status

Accepted

## Context

Data-plane quota reads need the same credential-scope separation as inference
admission. `prodex-authz` already exposes `DataPlaneQuota` and the
`data_plane_boundary_for_requirement` resolver, but gateway core did not have a
side-effect-free quota-read authorization plan that adapters could use before
reading or returning budget state.

Without that boundary, quota adapters could duplicate authorization matching or
accidentally authorize by resource/action alone, allowing control-plane or
break-glass credentials to become quota bypasses.

## Decision

Add `plan_gateway_quota_read_authorization` to `prodex-gateway-core`.

The planner resolves `Budget/Read`, `CredentialScope::DataPlane`, and
`Role::Operator` through `data_plane_boundary_for_requirement`, authorizes the
tenant-scoped resource with `DataPlaneQuota`, verifies the resolved tenant
matches the request tenant, and emits a gateway authorization span.

Gateway quota authorization failures use stable redacted response plans,
including `gateway_quota_authorization_unavailable` if the built-in data-plane
quota requirement cannot resolve to a boundary.

## Consequences

Quota read adapters can share one gateway-core authorization boundary before
touching budget state. Data-plane quota checks stay separate from inference
admission while preserving the same credential-scope and tenant-isolation
contracts.
