# ADR 0466: Gemini fallback response IDs use UUIDv7

## Status

Accepted.

## Context

When Gemini generate responses did not include an upstream response ID, the
Responses compatibility translator generated `resp_gemini_<runtime request
sequence>`. Runtime request sequence values are local to one proxy process and
are weaker than the enterprise globally unique ID boundary.

## Decision

Generate missing Gemini buffered response IDs with the typed domain `RequestId`
UUIDv7 generator while keeping the `resp_gemini_` prefix for compatibility with
existing response parsing and diagnostics.

## Consequences

Fallback Gemini response IDs no longer depend on process-local request sequence
values.
