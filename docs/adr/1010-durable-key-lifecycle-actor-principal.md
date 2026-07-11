# ADR 1010: Durable key lifecycle actor principal

## Status

Accepted.

## Context

Phase 0 identity requirements require security-sensitive control-plane actions
to use a canonical principal. The durable SQLite/PostgreSQL mounted virtual-key
lifecycle path still generated fresh `PrincipalId` values for the acting admin
and secret-reference command on every create or rotate plan. That avoided
process-local counters, but it prevented stable audit correlation for repeated
operations by the same compatibility admin identity.

## Decision

Durable virtual-key lifecycle planning now derives a stable compatibility
`PrincipalId` from the authenticated admin name and tenant using SHA-256 with a
fixed Prodex namespace. The derived ID is used for the control-plane actor
principal and the compatibility secret-reference principal at this mounted
admin boundary. The virtual key identity itself still follows ADR 1006 and must
be a typed `VirtualKeyId`.

## Consequences

Repeated durable virtual-key create or rotate operations by the same
compatibility admin in the same tenant now share a stable actor principal
instead of producing random principals per request. Different admin names or
tenants map to different actor IDs. This is a compatibility bridge, not the
final identity model: mounted admin authentication still needs to carry a
first-class canonical `PrincipalId` from tokens, SSO, and OIDC before the
gateway can fully satisfy the canonical principal requirement. Regression
coverage lives in
`crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gateway_admin_keys.rs`.
