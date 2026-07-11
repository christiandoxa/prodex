# ADR 0014: OIDC discovery and JWKS last-known-good cache

## Status

Accepted.

## Context

The enterprise gateway target requires OIDC discovery/JWKS caching with safe
behavior when the identity provider is temporarily unavailable. Failing closed on
unknown or malformed tokens remains required, but a transient IdP/JWKS outage
should not immediately break tokens that can still be verified with previously
fetched keys.

## Decision

Gateway OIDC HTTP cache entries are treated as last-known-good material on the
request path. Request-path authentication uses an existing cached discovery/JWKS
payload directly and fails closed when no usable cache exists. Metadata fetches
belong to startup prefetch or future bounded background refresh, not synchronous
admin request handling.

## Consequences

- Temporary JWKS/discovery outages do not immediately break already verifiable
  OIDC admin tokens.
- Unknown key IDs still fail because the stale JWKS must contain the token `kid`.
- A future gateway/control-plane split should add asynchronous refresh, HTTP
  cache-control parsing, bounded backoff, key-age metrics, and explicit
  stale-age limits.
