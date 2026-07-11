# ADR 0996: Copilot profile debug redaction

## Status

Accepted.

## Context

Copilot profile authentication and OAuth pool state carry profile names, API
keys, upstream API URLs, model catalogs, rotation cursors, and response affinity
bindings. Derived `Debug` output can leak provider credentials, account
metadata, model catalogs, or response/profile affinity keys through panic
output, assertions, or diagnostics.

## Decision

`RuntimeCopilotProfileAuth` and `RuntimeCopilotOAuthPoolState` use hand-written
`Debug` implementations that preserve the DTO shape and collection sizes while
redacting profile identifiers, API keys, URLs, model catalog values, pool
cursors, and response/profile bindings.

## Consequences

Copilot provider selection and affinity behavior are unchanged. Diagnostics can
still identify profile and pool DTOs without exposing credentials or affinity
metadata. Regression coverage lives in
`crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_copilot_tests.rs`.
