# ADR 1005: Durable key lifecycle tenant fail-closed

## Status

Accepted.

## Context

Phase 0 tenant-isolation requirements call out optional `tenant_id` values as a
multi-tenant risk. The durable SQLite/PostgreSQL admin virtual-key lifecycle
path converted missing or malformed key tenant IDs into a fresh `TenantId`
before planning the control-plane secret-reference operation. That made the
control-plane plan appear scoped while the stored compatibility key could still
remain unscoped or scoped to an invalid tenant string.

## Decision

Durable admin virtual-key lifecycle planning now requires the stored
`tenant_id` to be present and parse as a typed `TenantId`. Missing tenant IDs
return `gateway_key_tenant_required`; malformed tenant IDs return
`gateway_key_tenant_invalid`. File and Redis compatibility stores keep the
existing behavior because they do not run the durable secret-reference
lifecycle plan.

## Consequences

SQLite/PostgreSQL-backed admin key create and rotate operations fail closed
instead of synthesizing a random tenant for control-plane lifecycle planning.
Existing unscoped compatibility keys can still be read, but must be assigned a
valid tenant before durable lifecycle operations can proceed. Rollback is to
restore the previous random fallback only for single-tenant compatibility
deployments; do not use that rollback in regulated multi-tenant mode.
Regression coverage lives in
`crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gateway_admin_keys.rs`.
