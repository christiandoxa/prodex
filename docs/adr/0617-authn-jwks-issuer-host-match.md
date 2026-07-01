# 0617. Authn JWKS URL Must Match Issuer Host

## Status

Accepted

## Context

OIDC/JWKS refresh planning is already background-only and HTTPS-only. A
configured JWKS URL was still accepted when its host differed from the validated
issuer host.

That is an unnecessary trust expansion for the default enterprise path. It can
turn bad configuration into SSRF or token-key confusion before a transport
adapter fetches remote key material.

## Decision

`prodex-authn` now rejects configured JWKS refresh URLs whose host differs from
the validated OIDC issuer host.

The denial uses the same redacted temporary-authentication-unavailable response
family as other JWKS refresh-source failures.

## Consequences

- Default OIDC refresh planning is fail-closed for cross-host JWKS targets.
- Request-path authentication remains network-free.
- Deployments that need CDN or delegated JWKS hosts require a future explicit
  trust-policy exception instead of silently widening the refresh source.
