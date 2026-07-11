# ADR 0148: Secret store stable error responses

## Status

Accepted

## Context

`prodex-secret-store` owns development file-backed secrets, keyring selection,
secret revision probes, and refresh lease coordination. Raw `SecretError` and
`RefreshLeaseError` values can include filesystem paths, keyring service or
account names, permission details, and refresh-token-derived lock paths.

These details are useful for trusted diagnostics but must not become generic
client-visible or operator-facing API response text in enterprise deployments.

## Decision

Add stable redacted response planners:

- `plan_secret_error_response`
- `plan_refresh_lease_error_response`

Both planners map raw secret-store failures to generic service-unavailable
responses with stable machine-readable codes.

## Consequences

Future composition roots and control-plane endpoints can report secret store and
refresh lease failures without leaking filesystem paths, keyring identifiers,
secret names, or lock coordination internals.
