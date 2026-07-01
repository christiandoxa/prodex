# ADR 0689: Gateway State URL Environment References Match Exactly

## Status

Accepted.

## Context

`gateway.state.postgres_url_env` and `gateway.state.redis_url_env` identify
environment variables that hold durable state backend URLs. Policy validation
only rejected trim-empty values, and runtime config resolution trimmed the
environment variable names before reading connection URLs.

## Decision

Gateway state URL environment references must be exact non-empty values without
whitespace. Policy validation rejects whitespace-bearing references, and direct
runtime config resolution fails closed instead of trimming or falling back to a
default environment variable.

## Consequences

Canonical `PRODEX_GATEWAY_POSTGRES_URL` and `PRODEX_GATEWAY_REDIS_URL`
configuration remains unchanged. Padded state backend URL references no longer
resolve to credential-bearing connection URLs through trim-normalization.
