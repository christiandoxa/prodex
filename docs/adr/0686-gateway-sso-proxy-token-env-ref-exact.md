# ADR 0686: Gateway SSO Proxy Token Environment Reference Matches Exactly

## Status

Accepted.

## Context

`gateway.sso.proxy_token_env` is a secret-reference input for trusted SSO proxy
authentication. Policy validation only rejected trim-empty values, and runtime
config resolution trimmed the environment variable name before reading the
token.

## Decision

The SSO proxy token environment reference must now be an exact non-empty value
without whitespace. Runtime config resolution no longer trims
`gateway.sso.proxy_token_env`; whitespace-bearing references fail closed,
including when settings are built directly.

## Consequences

Canonical environment variable names are unchanged. Padded SSO proxy token
references no longer resolve to credential material through trim-normalization.
