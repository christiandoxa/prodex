# ADR 1015: OIDC user claim fail-closed

## Status

Accepted.

## Context

Gateway OIDC admin authentication derives the admin principal from the
configured user claim and then falls back to common claims such as `email`,
`preferred_username`, and `sub`. If an identity claim was present but malformed,
the resolver treated it the same as a missing claim and could use a later
fallback identity. That could silently change the authenticated principal and
weaken audit attribution.

## Decision

When an OIDC user identity claim is present, it must be an exact non-empty
string without whitespace. A malformed configured user claim now rejects admin
authentication. Fallback claims remain available only when earlier identity
claims are absent; a malformed fallback claim also rejects authentication
instead of skipping to a later identity claim.

## Consequences

OIDC admin identity derivation is fail-closed for explicit principal claims.
Deployments that intentionally rely on fallback identity claims should omit
earlier identity claims from the token instead of sending invalid values.
Regression coverage lives in
`crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gateway_admin_auth.rs`.
