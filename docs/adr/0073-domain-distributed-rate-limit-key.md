# ADR 0073: Domain distributed rate-limit keys

## Status

Accepted.

## Context

The enterprise target requires rate limits that are atomic across gateway
replicas and tenant-scoped in database/cache keys. `prodex-domain` already
modeled rate-limit admission decisions, but it did not provide a tenant-owned
bucket key or an atomic update DTO suitable for Redis Lua scripts, SQL updates,
or another distributed counter backend.

## Decision

Add `RateLimitBucketKey`, `RateLimitAtomicUpdate`, and
`RateLimitAtomicUpdateError`. Bucket keys always include `tenant_id`, may include
a `VirtualKeyId`, and include the window start. Atomic updates can only be built
from admitted requests whose tenant matches the bucket and whose increment is
non-zero.

## Consequences

- Storage adapters can perform atomic increment/expire operations using a common
  tenant-scoped key shape.
- Local in-memory admission can be replaced incrementally without changing the
  domain admission result model.
- Backend implementations still need to use actual atomic primitives (SQL
  conditional updates, Redis Lua, or equivalent); this ADR defines the portable
  domain contract they must honor.
