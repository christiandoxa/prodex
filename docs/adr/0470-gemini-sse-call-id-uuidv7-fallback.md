# ADR 0470: Gemini SSE fallback call IDs use UUIDv7

## Status

Accepted.

## Context

Gemini streaming tool-call translation generated missing call IDs from the
runtime request sequence and part index. Runtime request sequence values are
local to one proxy process and do not satisfy the enterprise globally unique
`CallId` boundary.

## Decision

Generate missing Gemini SSE tool call IDs with the typed domain `CallId` UUIDv7
generator while preserving the `call_gemini_` prefix. Repeated chunks for the
same in-progress tool call continue reusing the initially generated fallback ID.

## Consequences

Streaming Gemini fallback call IDs no longer depend on process-local request
sequence values.
