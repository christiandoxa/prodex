# ADR 1008: Durable SCIM lifecycle principal ID fail-closed

## Status

Accepted.

## Context

Phase 0 identifier requirements require tenant-owned identities to use typed,
globally unique IDs. The durable SQLite/PostgreSQL mounted SCIM lifecycle path
planned user create, update, and delete operations with a freshly generated
`PrincipalId` for the target user lifecycle command. That made the storage plan
target an identity that was not necessarily the compatibility SCIM user's
stored `user_id`.

## Decision

Durable SCIM lifecycle planning now requires the SCIM user's `user_id` to be
present and parse as a typed `PrincipalId`. Missing IDs return
`gateway_scim_user_id_required`; malformed IDs return
`gateway_scim_user_id_invalid`. File and Redis compatibility stores keep
existing behavior because they do not run the durable user-lifecycle plan.

## Consequences

SQLite/PostgreSQL-backed SCIM create, update, and delete operations fail closed
instead of synthesizing a random target principal for lifecycle planning.
Existing durable compatibility callers must provide a valid principal UUID in
`user_id` before SCIM lifecycle operations can proceed. Rollback is to restore
the generated target principal only for legacy single-tenant compatibility
deployments; do not use that rollback in regulated multi-tenant mode. The
admin actor identity is still generated at the compatibility edge until mounted
admin authentication carries a canonical `PrincipalId`. Regression coverage
lives in
`crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gateway_admin_scim.rs`.
