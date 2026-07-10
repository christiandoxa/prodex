# ADR 0991: Anthropic OAuth profile debug redaction

## Status

Accepted.

## Context

Anthropic OAuth profile auth records carry profile names and access tokens.
Derived `Debug` output can leak those credentials through panic output,
assertions, or runtime diagnostics.

## Decision

`RuntimeAnthropicOAuthProfileAuth` uses a hand-written `Debug` implementation
that preserves the profile-auth DTO shape while redacting profile and
access-token fields.

## Consequences

Runtime auth behavior is unchanged. Diagnostics can still identify the DTO type
without exposing OAuth credentials. Regression coverage lives in
`crates/prodex-app/src/runtime_launch/proxy_startup/anthropic_rewrite.rs`.
