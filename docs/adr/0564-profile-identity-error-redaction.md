# ADR 0564: Profile identity fallback errors are redacted

## Status

Accepted

## Context

Profile identity lookup can combine stored auth-secret parsing failures with
quota fallback failures when a profile email or account identity cannot be
resolved. Those detailed error chains are useful locally, but they can include
secret-like values from auth paths, provider URLs, bearer tokens, or quota
diagnostics.

## Decision

Before composing profile identity fallback errors, pass auth and quota error
chains through the shared secret-like text redactor. Keep the stable operator
context that identifies whether auth parsing, quota email fallback, or non-OpenAI
provider fallback failed.

## Consequences

- Client-visible and CLI-visible identity fallback errors no longer expose
  secret-like material from lower-level auth or quota failures.
- Existing fallback behavior and provider selection semantics are unchanged.
- Regression coverage pins the helper used by profile identity error assembly.
