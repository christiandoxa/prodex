# ADR 1000: Mem0 secrets debug redaction

## Status

Accepted.

## Context

Managed Mem0 setup persists PostgreSQL, admin API, and JWT secrets in a local
environment file. Derived `Debug` output for the secret bundle can leak those
credentials through panic output, assertions, or diagnostics.

## Decision

`Mem0Secrets` uses a hand-written `Debug` implementation that preserves the
secret bundle shape while redacting the PostgreSQL password, admin API key, and
JWT secret.

## Consequences

Managed Mem0 startup and environment rendering behavior are unchanged.
Diagnostics can still identify the secret bundle without exposing generated or
persisted Mem0 credentials. Regression coverage lives in
`crates/prodex-app/src/app_commands/mem0_memory.rs`.
