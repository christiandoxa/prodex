# ADR 0999: Copilot import auth debug redaction

## Status

Accepted.

## Context

Copilot profile import and runtime API authentication DTOs carry GitHub host
names, account logins, OAuth/import tokens, runtime API keys, and provider model
catalogs. Derived `Debug` output can leak credentials, account identifiers, or
model catalog metadata through panic output, assertions, or diagnostics.

## Decision

`CopilotRuntimeApiAuth` and `CopilotImportContext` use hand-written `Debug`
implementations that preserve the DTO shape and catalog size while redacting
tokens, runtime API keys, host names, account logins, and model catalog values.

## Consequences

Copilot profile import and runtime model discovery behavior are unchanged.
Diagnostics can still identify the auth/import DTOs without exposing
credentials or account metadata. Regression coverage lives in
`crates/prodex-app/src/profile_commands/copilot.rs`.
