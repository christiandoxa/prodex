# ADR 0580: Runtime Prefetch Error Redaction

## Status

Accepted

## Context

Runtime response prefetch reads upstream chunks before the local streaming
handoff. When upstream chunk reads fail, the same error text is stored in
terminal prefetch state, written to the runtime log, and queued for lookahead
or stream handoff handling.

Raw upstream error text can include copied headers, bearer tokens, key-bearing
URLs, or provider diagnostic material. The prefetch path should preserve
transport behavior while avoiding secret persistence.

## Decision

Normalize upstream prefetch error text once in the worker, then reuse the
redacted single-line value for terminal state, runtime log fields, and queued
prefetch error chunks.

## Consequences

- Prefetch diagnostics keep error kind, request, transport, and failure stage.
- Secret-like bearer tokens and key assignments are removed before runtime log
  persistence or downstream diagnostic propagation.
- Prefetch chunking, lookahead, backpressure, and transport classification
  behavior are unchanged.
