# ADR 1007: Durable SCIM lifecycle tenant fail-closed

## Status

Accepted.

## Context

Phase 0 tenant-isolation requirements call out optional `tenant_id` values as a
multi-tenant risk. The durable SQLite/PostgreSQL mounted SCIM lifecycle path
planned user create, update, and delete operations with a freshly generated
`TenantId` instead of the stored SCIM user's tenant. That made the
control-plane lifecycle plan scoped to an identity that did not necessarily
match the tenant-owned compatibility user.

## Decision

Durable SCIM lifecycle planning now requires the SCIM user's `tenant_id` to be
present and parse as a typed `TenantId`. Missing tenant IDs return
`gateway_scim_user_tenant_required`; malformed tenant IDs return
`gateway_scim_user_tenant_invalid`. File and Redis compatibility stores keep
existing behavior because they do not run the durable user-lifecycle plan.

## Consequences

SQLite/PostgreSQL-backed SCIM create, update, and delete operations fail closed
instead of synthesizing a random tenant for control-plane lifecycle planning.
Existing durable compatibility callers must provide a valid tenant UUID before
SCIM lifecycle operations can proceed. Rollback is to restore the generated
tenant only for legacy single-tenant compatibility deployments; do not use that
rollback in regulated multi-tenant mode. The target `PrincipalId` requirement
is handled separately in
`docs/adr/1008-durable-scim-lifecycle-principal-id-fail-closed.md`.
Regression coverage lives in
`crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gateway_admin_scim.rs`.
