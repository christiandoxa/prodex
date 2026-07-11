# ADR 0975: Gateway OIDC background refresh loop

## Status

Accepted

## Context

The legacy gateway admin OIDC adapter already kept discovery and JWKS network
fetches off the request path by reading only from a startup cache. That still
left two problems:

- gateway startup could still own the only refresh attempt; and
- stale cache entries never revalidated unless the process restarted.

The enterprise authn boundary already plans JWKS refreshes for background mode.
The compatibility adapter needed a smaller step that moves refreshes off the
startup path without changing request-path authentication semantics.

## Decision

Run legacy gateway OIDC discovery/JWKS refreshes on a bounded background worker.

- The worker performs the initial prefetch attempt after startup instead of on
  the startup path.
- Request-path authentication waits only for that initial bounded local prefetch
  attempt to finish, then reads cached discovery/JWKS data or fails closed.
- After the initial attempt, the same worker revalidates stale cached OIDC
  metadata in the background based on upstream `Cache-Control: max-age`
  metadata when present, falling back to
  `PRODEX_GATEWAY_OIDC_HTTP_CACHE_TTL_SECONDS` (default `300`).
- Request-path authentication may use expired discovery/JWKS data only inside a
  bounded last-known-good window. Upstream `Cache-Control:
  stale-while-revalidate` sets that window when present; otherwise the gateway
  uses `PRODEX_GATEWAY_OIDC_LAST_KNOWN_GOOD_SECONDS` (default `86400`).
- A TTL of `0` keeps request-path authentication network-free while forcing
  immediate background revalidation loops for tests and diagnostics.
- Failed refresh attempts retry after bounded backoff controlled by
  `PRODEX_GATEWAY_OIDC_REFRESH_FAILURE_BACKOFF_MS` (default `30000`).
- When native OIDC is configured, OIDC timing env overrides fail closed during
  gateway launch if they are empty, contain whitespace, are non-integer, or use
  zero for a setting that must be positive.
- Refresh success/failure/backoff and cached JWKS age are emitted with the
  existing low-cardinality observability metric names and labels in runtime
  logs.

## Consequences

- Gateway startup no longer blocks on identity-provider discovery or JWKS
  fetches.
- Request-path authentication remains network-free and fail-closed.
- Stale cached OIDC metadata can now refresh without a process restart, and
  cannot be used forever when the identity provider stays unavailable.
- Operators can correlate refresh success/failure/backoff and key age without
  exposing issuer URLs, JWKS URLs, key IDs, tenants, or users as metric labels.
- The remaining migration target is moving refresh ownership from this legacy
  gateway worker to a dedicated control-plane-managed refresh service with
  revisioned last-known-good state.
