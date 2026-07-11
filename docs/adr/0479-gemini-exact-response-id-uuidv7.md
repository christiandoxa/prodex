# ADR 0479: Gemini exact response IDs use UUIDv7

## Status

Accepted.

## Context

The Gemini exact-output short-circuit generated synthetic response IDs from the
runtime request sequence. Runtime request sequence values are local to one proxy
process and can collide across multi-replica gateway deployments.

## Decision

Generate synthetic Gemini exact-output response IDs with the typed domain
`RequestId` UUIDv7 generator while preserving the existing
`resp_gemini_exact_` prefix.

## Consequences

Exact-output Gemini compatibility responses no longer depend on process-local
request counters.
