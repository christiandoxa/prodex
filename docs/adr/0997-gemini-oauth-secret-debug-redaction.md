# ADR 0997: Gemini OAuth secret debug redaction

## Status

Accepted.

## Context

Gemini OAuth secret, token response, and user-info response DTOs carry access
tokens, refresh tokens, token metadata, expiry timestamps, account email
addresses, and project IDs. Derived `Debug` output can leak provider
credentials or account metadata through panic output, assertions, or
diagnostics.

## Decision

`GeminiOAuthSecret`, `GeminiTokenResponse`, and `GeminiUserInfoResponse` use
hand-written `Debug` implementations that preserve the DTO shape while
redacting token values, token metadata, expiry timing, email addresses, project
IDs, and auth-mode strings.

## Consequences

Gemini login, refresh, quota, and Code Assist setup behavior are unchanged.
Diagnostics can still identify OAuth DTOs without exposing persisted secrets,
fresh token exchange payloads, or user-info account metadata. Regression
coverage lives in
`crates/prodex-app/src/runtime_gemini_auth.rs` and
`crates/prodex-app/src/runtime_gemini_auth/oauth.rs`.
