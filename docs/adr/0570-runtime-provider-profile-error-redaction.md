# ADR 0570: Redact runtime provider profile errors

## Status

Accepted

## Context

Runtime launch aggregates profile-specific provider auth failures when no usable
Anthropic, Copilot, or Gemini OAuth profile is available. Those aggregate
messages previously embedded detailed `{err:#}` chains directly. Provider auth
refresh errors can include bearer tokens, key-bearing URLs, or provider
diagnostics.

## Decision

Keep the aggregate provider-profile error shape, including the profile name, but
redact each detailed error chain before joining it into the final launch error.

## Consequences

- Runtime launch still reports which profile failed.
- Secret-like provider auth material is removed from aggregated launch errors.
- Provider selection, refresh, auto-rotate, and fallback behavior are unchanged.
