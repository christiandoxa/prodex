# ADR 0688: Gateway Observability Environment Reference Matches Exactly

## Status

Accepted.

## Context

`gateway.observability.http_bearer_token_env` is a secret-reference input for
the observability HTTP sink bearer token. Policy validation only rejected
trim-empty values, and runtime config resolution trimmed the environment
variable name before reading the token.

## Decision

The observability HTTP bearer-token environment reference must now be an exact
non-empty value without whitespace. Runtime config resolution no longer trims
`gateway.observability.http_bearer_token_env`; whitespace-bearing, missing, or
empty references fail closed when settings are built directly.

## Consequences

Canonical environment variable names are unchanged. Padded observability bearer
token references no longer resolve to credential material through
trim-normalization, and configured HTTP sink authentication is no longer
silently disabled when the referenced secret is absent.
