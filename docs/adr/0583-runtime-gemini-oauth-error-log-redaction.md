# ADR 0583: Runtime Gemini OAuth Error Log Redaction

## Status

Accepted

## Context

Gemini local rewrite can refresh OAuth auth state and run background quota
status probes. Failure logs from those paths are useful for diagnosing profile
health, but raw error text can include copied headers, bearer tokens,
key-bearing URLs, or provider diagnostic material.

These paths should preserve profile selection, retry, and quota behavior while
avoiding secret persistence in runtime logs.

## Decision

Route Gemini OAuth auth-refresh and quota-pool error log values through local
helpers that redact secret-like text and keep structured log fields single-line.

## Consequences

- Gemini OAuth diagnostics keep request, provider, profile, and quota context.
- Secret-like bearer tokens and key assignments are removed before runtime log
  persistence.
- Gemini auth refresh, quota probing, and provider fallback behavior are
  unchanged.
