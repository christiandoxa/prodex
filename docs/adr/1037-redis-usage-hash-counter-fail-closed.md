# ADR 1037: Redis usage hash counter fail-closed parsing

## Status

Accepted.

## Context

The compatibility Redis usage backend stores virtual-key usage counters in
per-key hashes instead of a whole usage JSON map. Loading those hashes used to
parse numeric counter fields with a fallback to zero.

Affected symbols:

- `runtime_gateway_redis_usage_load`
- `runtime_gateway_redis_usage_from_hash`
- `runtime_gateway_redis_hash_u64`

The risk is accounting corruption: malformed Redis hash fields could erase
loaded request, token, or spend counters for admission and reporting.

## Decision

Redis usage hash counter values remain optional for missing legacy fields, but
present values must be exact unsigned integers without whitespace. Malformed
present values now fail the Redis usage load instead of silently becoming zero.

## Consequences

Corrupt Redis compatibility usage state is visible in startup/runtime logs and
does not silently weaken budget/admission decisions. Empty or missing legacy
fields still load as zero for backward compatibility.

Regression coverage:

- `redis_usage_hash_helpers_round_trip_usage_fields`
- `redis_usage_hash_rejects_malformed_counter_fields`
