# ADR 0589: Gateway OIDC Prefetch Timeout

## Status

Accepted

## Context

Gateway admin OIDC authentication must not perform discovery or JWKS fetches on
the request path. The legacy compatibility gateway therefore prefetches OIDC
metadata at startup, but that prefetch used the shared blocking HTTP client
without a request timeout specific to OIDC metadata fetches.

## Decision

Apply a bounded request timeout to startup OIDC discovery/JWKS prefetches. The
timeout defaults to 2000 ms and can be overridden with
`PRODEX_GATEWAY_OIDC_PREFETCH_TIMEOUT_MS`.

## Consequences

- Slow or unavailable identity providers cannot stall gateway startup
  indefinitely during compatibility prefetch.
- Admin request-path OIDC authentication remains network-free and fails closed
  when the startup cache is unavailable.
- The broader migration target remains moving legacy startup prefetch into a
  bounded background control-plane refresh worker.
