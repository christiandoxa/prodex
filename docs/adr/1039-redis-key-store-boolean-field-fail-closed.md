# ADR 1039: Redis key-store boolean field fail-closed parsing

## Status

Accepted.

## Context

The compatibility Redis key store persists boolean governance state in hashes,
including virtual-key `disabled` and SCIM user `active`. The loader previously
treated every value except `1` as `false`.

Affected symbols:

- `runtime_gateway_redis_stored_key_from_hash`
- `runtime_gateway_redis_scim_user_from_hash`
- `runtime_gateway_redis_hash_bool`

The risk is fail-open key governance: a malformed `disabled` field could load
as `false` and re-enable a key whose persisted state is corrupt.

## Decision

Missing or empty legacy boolean fields still load as `false`, but present
boolean values must be exact `0` or `1` without whitespace. Malformed present
values now fail Redis key-store loading.

## Consequences

Corrupt Redis compatibility key-store records do not silently change
authorization state. Legacy records that omit boolean fields keep their previous
default.

Regression coverage:

- `redis_key_store_hash_rejects_malformed_bool_fields`
