# ADR 0998: Claude OAuth debug redaction

## Status

Accepted.

## Context

Claude OAuth secrets, credentials-file DTOs, nested credential tokens, and auth
status values carry access tokens, expiry timestamps, subscription/auth method
labels, and account identifiers. Derived `Debug` output can leak provider
credentials or account metadata through panic output, assertions, or
diagnostics.

## Decision

`ClaudeOAuthSecret`, `ClaudeAuthStatus`, `ClaudeCredentialsFile`, and
`ClaudeCredentialsToken` use hand-written `Debug` implementations that preserve
the DTO shape and login boolean while redacting tokens, expiry timing,
subscription/auth method labels, and account identifiers.

## Consequences

Claude credential parsing, refresh checks, login status parsing, and profile
identity behavior are unchanged. Diagnostics can still identify Claude auth DTOs
without exposing credentials or account metadata. Regression coverage lives in
`crates/prodex-app/src/runtime_claude_auth.rs`.
