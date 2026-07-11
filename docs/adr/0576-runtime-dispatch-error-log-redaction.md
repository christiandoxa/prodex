# ADR 0576: Runtime Dispatch Error Log Redaction

## Status

Accepted

## Context

Runtime proxy dispatch logs include detailed local error chains for capture,
rewrite, transport, Anthropic compatibility, and websocket session failures.
Those chains are useful for operators, but they can include copied header
values, bearer tokens, key-bearing URLs, or provider diagnostic text.

The client-facing responses are already stable. The remaining risk was the
runtime log boundary.

## Decision

Route runtime dispatch error log values through the shared secret-like text
redactor before writing structured log fields.

## Consequences

- Operators still see the failing event, request ID, transport, and redacted
  error context.
- Secret-like bearer tokens and key assignments are removed before runtime log
  persistence.
- Provider retry, affinity, streaming, and client response behavior are
  unchanged.
