# ADR 1001: Kiro auth debug redaction

## Status

Accepted.

## Context

Kiro profile import and auth secret DTOs carry auth keys, auth kinds, raw auth
JSON, access tokens, refresh tokens, account emails, AWS profile ARNs, profile
names, start URLs, and regions. Derived `Debug` output can leak provider
credentials or account metadata through panic output, assertions, or
diagnostics.

## Decision

`KiroImportContext` and `KiroAuthSecret` use hand-written `Debug`
implementations that preserve the DTO shape while redacting auth keys, auth
kinds, raw auth JSON, identity metadata, profile selectors, start URLs, and
regions.

## Consequences

Kiro profile import, CLI data materialization, and model catalog snapshot
behavior are unchanged. Diagnostics can still identify Kiro auth DTOs without
exposing credentials or account metadata. Regression coverage lives in
`crates/prodex-app/src/profile_commands/kiro.rs`.
