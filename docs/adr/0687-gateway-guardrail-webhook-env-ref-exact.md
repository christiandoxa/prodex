# ADR 0687: Gateway Guardrail Webhook Environment Reference Matches Exactly

## Status

Accepted.

## Context

`gateway.guardrails.webhook_bearer_token_env` is a secret-reference input for
the guardrail webhook bearer token. Policy validation only rejected trim-empty
values, and runtime config resolution trimmed the environment variable name
before reading the token.

## Decision

The guardrail webhook bearer-token environment reference must now be an exact
non-empty value without whitespace. Runtime config resolution no longer trims
`gateway.guardrails.webhook_bearer_token_env`; whitespace-bearing references
fail closed or are ignored when settings are built directly.

## Consequences

Canonical environment variable names are unchanged. Padded guardrail webhook
token references no longer resolve to credential material through
trim-normalization.
