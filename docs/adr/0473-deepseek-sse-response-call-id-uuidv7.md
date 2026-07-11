# ADR 0473: DeepSeek SSE fallback IDs use UUIDv7

## Status

Accepted.

## Context

DeepSeek streaming compatibility generated missing response and tool-call IDs
from the runtime request sequence and tool-call index. Those values are local to
one proxy process and can collide across multi-replica gateway deployments.

## Decision

Generate missing DeepSeek SSE response IDs with typed domain `RequestId` UUIDv7
values and missing tool-call IDs with typed domain `CallId` UUIDv7 values. Keep
the existing `resp_deepseek_` and `call_deepseek_` compatibility prefixes, and
reuse the generated call ID across later chunks for the same in-progress tool
call.

## Consequences

DeepSeek SSE fallback IDs no longer depend on process-local request sequence
values. Upstream-provided IDs are still preserved when present.
