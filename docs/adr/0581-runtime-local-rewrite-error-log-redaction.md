# ADR 0581: Runtime Local Rewrite Error Log Redaction

## Status

Accepted

## Context

The local provider rewrite gateway logs upstream request failures before
returning the stable `upstream request failed` response. The raw error chain can
include copied headers, bearer tokens, key-bearing URLs, or provider diagnostic
material.

Client-visible behavior was already stable. The remaining boundary was the
persisted runtime log value.

## Decision

Route local rewrite upstream error log values through a local helper that
redacts secret-like text and keeps the structured log field single-line.

## Consequences

- Operators still see the request ID, transport, and failure context.
- Secret-like bearer tokens and key assignments are removed before runtime log
  persistence.
- Provider rewrite routing, upstream behavior, and client response compatibility
  are unchanged.
