# 0163: Identity and JWKS Stable Error Responses

## Status

Accepted

## Context

The enterprise OIDC boundary validates issuer, audience, and algorithm policy
against cached JWKS state. The request path must remain network-free and may use
last-known-good keys during bounded backoff, but client-visible failures must
not reveal IdP topology, issuer/audience values, key IDs, key counts, cache
timing, or refresh errors.

Existing authentication adapters already have stable error boundaries, but the
domain identity primitives also need reusable response planners so future
gateway/control-plane composition roots do not accidentally expose raw
configuration, token-validation, or JWKS cache details.

## Decision

`prodex-domain` owns stable identity response planners:

- `plan_identity_config_error_response`
- `plan_token_validation_error_response`
- `plan_jwks_refresh_decision_error_response`

The planners return serializable status/code/message triples. They keep
configuration failures, token-validation failures, and unavailable JWKS cache
states machine-readable while redacting issuer, audience, key ID, cache age,
retry/backoff, and last-known-good internals.

Usable JWKS decisions such as fresh keys, refresh-now, stale-while-revalidate,
and last-known-good during backoff are not errors and therefore do not produce a
client error response.

## Consequences

- OIDC/JWKS adapters get a shared fail-closed response boundary.
- Last-known-good behavior remains internal and observable through diagnostics,
  not public API details.
- Future composition roots can expose stable authentication/readiness failures
  without leaking IdP or cache internals.
