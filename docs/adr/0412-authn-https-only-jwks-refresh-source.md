# 0412: Authn HTTPS-only JWKS refresh source

## Status

Accepted

## Context

OIDC JWKS refresh is intentionally planned outside the gateway request path,
but configured discovery and refresh URLs are still trust boundaries. Accepting
plaintext HTTP would allow key material to be fetched over an unauthenticated
transport in regulated multi-tenant deployments.

## Decision

`prodex-authn` now accepts configured JWKS refresh URLs only when they use
`https://`. Runtime policy validation also requires HTTPS OIDC issuers, because
issuer discovery derives a JWKS refresh source from that URL. Empty values,
whitespace, local file URLs, and plaintext `http://` URLs fail closed.

The stable client response remains the existing redacted authentication
temporary-unavailable envelope.

## Consequences

- Production OIDC refresh plans fail closed for plaintext discovery or JWKS
  transport.
- Development environments that need local keys must use an HTTPS endpoint or a
  preloaded cache snapshot.
