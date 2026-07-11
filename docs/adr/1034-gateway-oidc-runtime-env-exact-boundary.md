# ADR 1034: Gateway OIDC runtime env exact boundary

## Status

Accepted.

## Context

Gateway OIDC discovery/JWKS refresh uses runtime environment overrides for
prefetch timeout, HTTP cache TTL, refresh failure backoff, and last-known-good
window. These values control fail-closed authentication behavior and cache
freshness for admin identities. Previously malformed explicit values could fall
back to defaults in the legacy refresh helpers, making production wiring errors
hard to detect.

## Decision

When native gateway OIDC is configured, runtime launch validates:

- `PRODEX_GATEWAY_OIDC_PREFETCH_TIMEOUT_MS`
- `PRODEX_GATEWAY_OIDC_HTTP_CACHE_TTL_SECONDS`
- `PRODEX_GATEWAY_OIDC_REFRESH_FAILURE_BACKOFF_MS`
- `PRODEX_GATEWAY_OIDC_LAST_KNOWN_GOOD_SECONDS`

Configured values must be exact unsigned integers without whitespace. Prefetch
timeout and refresh failure backoff must be greater than zero. Cache TTL and
last-known-good window may be zero to preserve the existing diagnostic/test
escape hatch.

## Consequences

OIDC deployment mistakes now fail during gateway launch instead of silently
using default cache timing. Unset values keep the documented defaults.
