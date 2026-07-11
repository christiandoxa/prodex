# ADR 0472: Gemini Live fallback call IDs use UUIDv7

## Status

Accepted.

## Context

Gemini Live compatibility used the constant fallback `call_gemini_live` when an
upstream function call omitted an ID. That can collide across tool calls and
does not satisfy the enterprise globally unique `CallId` boundary.

## Decision

Generate missing Gemini Live tool call IDs with the typed domain `CallId` UUIDv7
generator while preserving the `call_gemini_live_` prefix.

## Consequences

Gemini Live fallback call IDs no longer collapse unrelated tool calls into one
constant identifier.
