# ADR 0582: Runtime Gemini Compact Error Log Redaction

## Status

Accepted

## Context

Gemini remote compact first tries a semantic compact request and falls back to
local compacting when the semantic request fails. The fallback log included the
raw error chain as `reason`, which can carry copied headers, bearer tokens,
key-bearing URLs, or provider diagnostic material.

The fallback behavior is intentionally preserved. The boundary to harden is the
persisted runtime log value.

## Decision

Route Gemini compact fallback reason values through a local helper that redacts
secret-like text and keeps the structured log field single-line.

## Consequences

- Operators still see that semantic compact failed and local fallback was used.
- Secret-like bearer tokens and key assignments are removed before runtime log
  persistence.
- Remote compact fallback behavior and response compatibility are unchanged.
