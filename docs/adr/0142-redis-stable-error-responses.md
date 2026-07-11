# ADR 0142: Redis stable error responses

## Status

Accepted

## Context

`prodex-storage-redis` plans short-lived cache, rate-limit, and coordination
operations for gateway and recovery-worker use cases. Raw `RedisPlanError`
values can include tenant IDs, key-shaping details, TTL policy failures, and
lease-owner validation details. Those are useful for trusted diagnostics but
must not be copied into API or worker-facing response envelopes.

## Decision

Add `plan_redis_error_response` to `prodex-storage-redis`. It maps
`RedisPlanError` into a stable, redacted `redis_coordination_unavailable`
response with service-unavailable status.

Application composition roots may adapt this storage-level response into
use-case-specific codes and messages while preserving the storage boundary's
redaction decision.

## Consequences

Redis-backed coordination failures now have one reusable redaction boundary.
Application-level recovery lease release responses can remain use-case specific
without inspecting raw Redis plan error variants.
