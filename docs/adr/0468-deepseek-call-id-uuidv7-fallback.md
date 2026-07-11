# ADR 0468: DeepSeek fallback call IDs use UUIDv7

## Status

Accepted.

## Context

When DeepSeek tool-call objects did not include an upstream call ID, the
Responses compatibility translator used the constant fallback `call_0`. That can
collide across tool calls, retries, and replicas, and does not satisfy the
enterprise globally unique `CallId` boundary.

## Decision

Generate missing DeepSeek tool call IDs with the typed domain `CallId` UUIDv7
generator while keeping a `call_deepseek_` prefix for provider diagnostics.

## Consequences

Fallback DeepSeek call IDs are unique enough for multi-replica compatibility and
no longer collapse multiple missing upstream IDs into `call_0`.
