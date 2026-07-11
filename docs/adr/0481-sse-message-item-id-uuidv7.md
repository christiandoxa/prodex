# ADR 0481: SSE message item IDs use UUIDv7

## Status

Accepted.

## Context

Gemini and DeepSeek SSE adapters emitted fallback message item IDs from the
local runtime request sequence. In multi-replica deployments, those process-local
values can collide and are not valid globally unique identifiers.

## Decision

Generate fallback SSE message, media, and citation item IDs from `RequestId`
UUIDv7 values when each stream state is created. Keep the generated IDs stable
for the lifetime of that stream.

## Consequences

Fallback SSE item IDs are globally unique across replicas. The internal
process-local request sequence can remain for local logs and request-scoped
bookkeeping.
