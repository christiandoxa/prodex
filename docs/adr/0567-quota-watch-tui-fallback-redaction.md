# ADR 0567: Redact quota watch TUI fallback diagnostics

## Status

Accepted

## Context

`prodex quota` can fall back from the interactive TUI to plain watch output when
terminal setup or TUI input fails and strict mode is disabled. The fallback
notice printed the detailed `{err:#}` chain directly to stderr. TUI setup errors
can include terminal, backend, or environment diagnostics with secret-like
material.

## Decision

Keep the existing fallback behavior and message, but pass the detailed TUI error
chain through the shared secret-like text redactor before printing it.

## Consequences

- Users still see that quota watch fell back to plain mode.
- Secret-like bearer tokens and key-bearing URLs are removed from the fallback
  stderr diagnostic.
- Strict mode and quota watch rendering behavior are unchanged.
