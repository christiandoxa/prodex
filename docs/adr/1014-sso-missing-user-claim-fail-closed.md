# ADR 1014: SSO missing user claim fail-closed

## Status

Accepted.

## Context

Gateway trusted-proxy SSO admin authentication requires a proxy token and then
derives the admin principal from a configured user header. When that header was
missing, the resolver synthesized `sso-admin`. A valid proxy token plus a
missing identity header could therefore create an implicit control-plane
principal, weakening audit attribution and privilege boundaries.

## Decision

Missing SSO user headers now fail closed. The resolver accepts only exact,
non-empty user header values without whitespace. Whitespace-padded, empty, and
missing values all reject SSO admin authentication.

## Consequences

Trusted proxy integrations must forward an explicit user identity header for
admin routes. Existing exact user header values keep working. This does not
change OIDC claim fallback behavior, which already requires one of the
configured/user/email/preferred-username/sub claims. Regression coverage lives
in
`crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gateway_admin_auth.rs`.
