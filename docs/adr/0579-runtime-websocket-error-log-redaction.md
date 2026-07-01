# ADR 0579: Runtime WebSocket Error Log Redaction

## Status

Accepted

## Context

Runtime WebSocket transport logs include connect pressure, proxy tunnel,
upstream read, and upstream send failures. These fields are useful for
operational diagnosis, but raw WebSocket or I/O error text can include copied
headers, bearer tokens, key-bearing URLs, or provider diagnostic material.

The proxy must preserve WebSocket retry, affinity, and transport semantics. The
remaining risk was only the persisted runtime log value.

## Decision

Route WebSocket error log values through a local helper that redacts
secret-like text and keeps each error field on one line.

## Consequences

- WebSocket failure logs keep request, profile, transport, stage, and retry
  context.
- Secret-like bearer tokens and key assignments are removed before runtime log
  persistence.
- WebSocket connect, reuse, retry, and failure classification behavior are
  unchanged.
