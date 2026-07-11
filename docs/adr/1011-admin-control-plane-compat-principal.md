# ADR 1011: Admin control-plane compatibility principal

## Status

Accepted.

## Context

Mounted gateway admin routes now delegate route, idempotency, precondition, and
write-role decisions through the shared control-plane planners. The adapter was
building those planner requests with fresh `TenantId` and `PrincipalId` values
on every call. That preserved legacy compatibility, but it made the planning
identity unstable and weakened audit/idempotency correlation for repeated
operations by the same admin credential.

## Decision

The mounted admin control-plane adapter now uses a stable compatibility tenant
and principal for planner requests. If the authenticated admin has a typed
tenant scope, that tenant is used. Otherwise the adapter derives a deterministic
compatibility tenant from the admin name. The actor principal is derived from
the resolved tenant and admin name using SHA-256 with fixed Prodex namespaces.

## Consequences

Repeated mounted admin route planning for the same compatibility admin now uses
stable IDs instead of random IDs. Typed tenant-scoped admins keep their real
tenant in planner requests. Legacy unscoped admins remain compatible through a
deterministic compatibility tenant, which is not the final enterprise identity
model. The remaining migration target is to carry canonical `PrincipalId` and
mandatory typed tenant context directly from admin tokens, SSO, and OIDC.
Regression coverage lives in
`crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gateway_admin_router.rs`.
