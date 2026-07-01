# ADR 0707: Gateway OIDC URL Exact Boundary

## Status

Accepted.

## Context

`gateway.sso.oidc_issuer`, `gateway.sso.oidc_jwks_url`, and
`gateway.sso.oidc_audience` define the trusted identity-provider boundary for
gateway SSO. Policy validation and runtime launch previously trimmed some of
these values before use, allowing padded inputs to be silently normalized into
accepted identity-provider endpoints or audience selectors.

## Decision

Treat gateway OIDC URL and audience fields as exact configuration boundaries.
They must be non-empty, contain no whitespace, and URL fields must use HTTPS.
Validation and runtime launch check the configured value directly instead of
trimming it first.

## Consequences

Operators must fix padded OIDC URL or audience values in `policy.toml`. Gateway
identity configuration remains auditable and fails closed instead of accepting
hidden normalization at the authentication boundary.
