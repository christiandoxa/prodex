# ADR 0469: Gemini SSE fallback response IDs use UUIDv7

## Status

Accepted.

## Context

Gemini streaming translation initialized fallback response IDs from the runtime
request sequence before any upstream `responseId` was observed. Runtime request
sequence values are local to one proxy process and are weaker than the
enterprise globally unique identifier boundary.

## Decision

Initialize Gemini SSE fallback response IDs with the typed domain `RequestId`
UUIDv7 generator while preserving the `resp_gemini_` prefix. If Gemini later
sends a concrete upstream response ID, the existing override behavior remains.

## Consequences

Streaming fallback response IDs no longer depend on process-local request
sequence values.
