# ADR 0150: Config cache window stable error responses

## Status

Accepted

## Context

The configuration boundary validates cache refresh, stale, and expiry windows
before a gateway or control-plane adapter can activate revisioned policy/config
state. Raw `ConfigCacheWindowError` values identify the internal timing
relationship that failed. That detail is useful for trusted operator diagnostics,
but client-visible readiness, admin, or validation APIs should not expose cache
window internals or encourage callers to infer last-known-good timing behavior.

## Decision

Add `plan_config_cache_window_error_response` to `prodex-config`. It maps every
cache-window validation failure into the same stable response plan:

- status: `InvalidConfiguration`;
- code: `configuration_cache_window_invalid`; and
- message: `configuration cache window is invalid`.

The raw error remains available for internal logs and audit diagnostics, while
the public response avoids refresh, stale, expiry, TTL, revision, and
last-known-good implementation details.

## Consequences

Composition roots can expose deterministic, machine-readable configuration cache
validation failures without leaking policy-cache timing internals. Future
policy/JWKS cache adapters should use this boundary before returning
client-visible validation errors.
