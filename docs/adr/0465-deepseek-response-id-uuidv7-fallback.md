# ADR 0465: DeepSeek fallback response IDs use UUIDv7

## Status

Accepted.

## Context

When DeepSeek chat responses did not include an upstream response ID, the
Responses compatibility translator generated `resp_deepseek_<runtime request
sequence>`. Runtime request sequence values are local to a proxy process and are
weaker than the enterprise globally unique ID boundary.

## Decision

Generate missing DeepSeek buffered response IDs with the typed domain `RequestId`
UUIDv7 generator while keeping the `resp_deepseek_` prefix for compatibility
with existing response parsing and diagnostics.

## Consequences

Fallback response IDs no longer depend on process-local request sequence values.
