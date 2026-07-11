# ADR 0990: Gemini OAuth profile debug redaction

## Status

Accepted.

## Context

Gemini OAuth profile auth records carry profile names, Codex home paths, account
email addresses, access tokens, and project IDs. Derived `Debug` output can leak
that credential and identity metadata through panic output, assertions, or
runtime diagnostics.

## Decision

`RuntimeGeminiOAuthProfileAuth` uses a hand-written `Debug` implementation that
preserves the profile-auth DTO shape while redacting profile, path, email,
access-token, and project fields.

## Consequences

Runtime auth behavior is unchanged. Diagnostics can still identify the DTO type
without exposing OAuth credentials or local account metadata. Regression
coverage lives in
`crates/prodex-app/src/runtime_launch/proxy_startup/gemini_rewrite.rs`.
