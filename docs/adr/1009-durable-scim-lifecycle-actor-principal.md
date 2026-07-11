# ADR 1009: Durable SCIM lifecycle actor principal

## Status

Accepted.

## Context

Phase 0 identity requirements require security-sensitive control-plane actions
to use a canonical principal. The durable SQLite/PostgreSQL mounted SCIM
lifecycle path still created a fresh `PrincipalId` for the acting admin on each
plan. That avoided process-local counters, but it prevented stable audit
correlation for repeated SCIM create, update, or delete operations by the same
compatibility admin identity.

## Decision

Durable SCIM lifecycle planning now derives a stable compatibility
`PrincipalId` from the authenticated admin name and tenant using SHA-256 with a
fixed Prodex namespace. The derived ID is used only for the control-plane actor
principal at the mounted compatibility boundary. The target SCIM user's
`user_id` still follows ADR 1008 and must be a typed `PrincipalId`.

## Consequences

Repeated durable SCIM lifecycle operations by the same compatibility admin in
the same tenant now share a stable actor principal instead of producing a random
actor per request. Different admin names or tenants map to different actor IDs.
This is a compatibility bridge, not the final identity model: mounted admin
authentication still needs to carry a first-class canonical `PrincipalId` from
tokens, SSO, and OIDC before the gateway can fully satisfy the canonical
principal requirement. Regression coverage lives in
`crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gateway_admin_scim.rs`.
