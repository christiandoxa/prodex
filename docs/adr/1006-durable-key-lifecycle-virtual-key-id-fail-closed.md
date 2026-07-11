# ADR 1006: Durable key lifecycle virtual-key ID fail-closed

## Status

Accepted.

## Context

Phase 0 identifier requirements require tenant-owned resources to use typed,
globally unique IDs. The durable SQLite/PostgreSQL admin virtual-key lifecycle
path already plans secret-reference operations against a `TenantStorageKey`,
but missing or malformed compatibility `virtual_key_id` values were converted
into a fresh `VirtualKeyId` before planning. That could make the control-plane
plan target an identity that was not the stored key identity.

## Decision

Durable admin virtual-key lifecycle planning now requires the stored
`virtual_key_id` to be present and parse as a typed `VirtualKeyId`. Missing IDs
return `gateway_key_id_required`; malformed IDs return
`gateway_key_id_invalid`. File and Redis compatibility stores keep existing
behavior because they do not run the durable secret-reference lifecycle plan.

## Consequences

SQLite/PostgreSQL-backed admin key create and rotate operations fail closed
instead of synthesizing a random virtual-key identity for lifecycle planning.
Existing compatibility keys with missing or malformed IDs can still be read,
but must be migrated or recreated with a valid `virtual_key_id` before durable
lifecycle operations can proceed. Rollback is to restore the random fallback
only for legacy single-replica compatibility deployments; do not use that
rollback in regulated multi-tenant mode. Regression coverage lives in
`crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gateway_admin_keys.rs`.
