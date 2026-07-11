# ADR 0803: Redact JWKS cache debug output

Status: Accepted

## Context

`JwksCacheSnapshot` carries key-cache freshness, stale-window, refresh-error,
and backoff timing state. Its derived `Debug` formatter exposed those raw
operational internals to diagnostics and containing debug output.

## Decision

Use a custom `Debug` implementation for `JwksCacheSnapshot` that preserves
whether usable key material is present while redacting timestamps, backoff
values, and raw key counts.

## Consequences

Diagnostics can still distinguish empty from populated JWKS caches, but raw
last-known-good and refresh-backoff internals no longer appear through
`JwksCacheSnapshot` debug output.
