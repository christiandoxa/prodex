# ADR 0471: Gemini Live response IDs use UUIDv7

## Status

Accepted.

## Context

Gemini Live compatibility generated local response and item IDs from the runtime
request sequence and turn sequence. Runtime request sequence values are local to
one proxy process and are weaker than the enterprise globally unique identifier
boundary.

## Decision

Generate Gemini Live response and item IDs from typed domain `RequestId` UUIDv7
values while preserving the existing `resp_gemini_live_` and `item_gemini_live_`
prefixes.

## Consequences

Gemini Live turn IDs no longer depend on process-local request sequence values,
and each turn still receives distinct response IDs.
