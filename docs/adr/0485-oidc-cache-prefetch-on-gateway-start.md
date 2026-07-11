# ADR 0485: OIDC cache is prefetched on gateway start

## Status

Accepted.

## Context

Native gateway OIDC auth discovered JWKS metadata lazily during the first admin
request. That placed blocking IdP network I/O on the request path.

## Decision

When OIDC is configured, the gateway attempts to prefetch discovery and JWKS
documents during proxy startup using the existing bounded cache. Startup
continues if prefetch fails and logs a stable, redacted error kind.

Request-path authentication reads only cached discovery/JWKS documents. It fails
closed when prefetch did not populate usable cache material instead of fetching
metadata synchronously.

## Consequences

Healthy deployments avoid first-request OIDC metadata fetch latency. Existing
last-known-good behavior remains available for stale cache refresh failures.
Failed prefetch attempts leave admin OIDC authentication denied but
network-free until cache material is populated outside the request path.
