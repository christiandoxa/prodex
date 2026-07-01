# ADR 0261: Tenant Lifecycle Storage Composition

## Status

Accepted

## Context

Tenant create and update operations already required control-plane
authorization and immutable audit, but the tenant registry itself did not have
a dedicated storage composition boundary. Without that boundary, adapters could
persist tenant metadata outside authorization/audit flow or duplicate
backend-specific SQL.

Tenant rows are also the root of tenant-owned foreign keys, so regulated
multi-tenant deployments need a durable, tenant-scoped registry that is updated
only after an authorized lifecycle decision.

## Decision

Add `TenantLifecycleCommand` and `plan_tenant_lifecycle` to `prodex-storage`.
The command stores the tenant storage key, tenant ID, display name, lifecycle
kind, and operation time. PostgreSQL and SQLite expose request-path DML plans
for upserting `prodex_tenants`; migration DDL remains in explicit migrator
plans.

Add `plan_application_tenant_lifecycle` to `prodex-application`. The planner
accepts `TenantCreate` and `TenantUpdate` control-plane actions on
`ResourceKind::Tenant`, verifies that the action tenant matches the command
tenant, verifies that the operation matches the lifecycle kind, and composes
append-only audit storage with backend tenant storage. Audit storage is always
planned for authorized and denied decisions; tenant storage is planned only
after authorization.

## Consequences

Tenant lifecycle changes now pass through one application boundary before
backend-specific persistence. Client-visible error responses stay redacted and
do not expose tenant IDs, display names, or SQL details.
