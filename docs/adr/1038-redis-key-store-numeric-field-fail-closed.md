# ADR 1038: Redis key-store numeric field fail-closed parsing

## Status

Accepted.

## Context

The compatibility Redis key store persists virtual-key budgets, rate limits,
and SCIM/key timestamps in per-record hashes. Loader helpers used to parse
numeric hash fields with trim-and-ignore behavior, allowing malformed present
values to disappear as `None` or `0`.

Affected symbols:

- `runtime_gateway_redis_load_key_store_from_conn`
- `runtime_gateway_redis_stored_key_from_hash`
- `runtime_gateway_redis_scim_user_from_hash`
- `runtime_gateway_redis_hash_u64`
- `runtime_gateway_redis_hash_optional_u64`

The risk is policy drift: malformed Redis budget, rate-limit, or timestamp
fields could silently weaken key governance or hide corrupted state.

## Decision

Redis key-store numeric hash fields remain backward compatible for missing or
empty legacy values, but present values must be exact unsigned integers without
whitespace. Malformed present values now fail the key-store load.

## Consequences

Corrupt Redis compatibility key-store records are visible instead of being
normalized into weaker runtime policy. Missing legacy numeric fields still load
with the previous empty/zero semantics.

Regression coverage:

- `redis_key_store_optional_u64_preserves_zero`
- `redis_key_store_hash_rejects_malformed_numeric_fields`
