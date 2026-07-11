# 0236: Redis Rate-Limit Result Contract

## Status

Accepted

## Context

Redis rate limiting is allowed only as a rebuildable distributed limiter. The
Lua script already performs atomic `GET`/`INCRBY`/`PEXPIREAT`, but adapter code
also needs a stable interpretation contract for the script result. Without one,
different adapters could disagree about allow/deny flags, current counts, or
Redis `PTTL` sentinel values.

## Decision

`prodex-storage-redis` owns `plan_redis_rate_limit_result`. It converts the Lua
tuple `{allowed, current_requests, ttl_ms}` into:

- `RedisRateLimitDecision::Allowed`;
- `RedisRateLimitDecision::Limited`;
- `RedisPlanError::InvalidRateLimitResult` for malformed tuples.

Redis `PTTL` sentinel values `-1` and `-2` become `None`; positive TTLs and zero
remain explicit millisecond values.

## Consequences

- Redis adapters can keep Lua execution thin and delegate result semantics to
  the storage boundary.
- Invalid limiter results fail closed behind the existing redacted Redis error
  response plan.
- Redis remains a limiter/cache/coordination dependency, not durable accounting
  state.
