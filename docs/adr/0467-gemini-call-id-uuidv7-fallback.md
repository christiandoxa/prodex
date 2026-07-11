# ADR 0467: Gemini fallback call IDs use UUIDv7

## Status

Accepted.

## Context

When Gemini function-call parts did not include an upstream call ID, the
Responses compatibility translator generated `call_gemini_<runtime request
sequence>_<index>`. Runtime request sequence values are local to one proxy
process and are weaker than the enterprise globally unique `CallId` boundary.

## Decision

Generate missing Gemini tool call IDs with the typed domain `CallId` UUIDv7
generator while keeping the `call_gemini_` prefix for compatibility with
existing tool-call parsing and diagnostics.

## Consequences

Fallback Gemini call IDs no longer depend on process-local request sequence
values.
