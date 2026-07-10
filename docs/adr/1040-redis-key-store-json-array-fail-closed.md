# ADR 1040: Redis key-store JSON array fail-closed parsing

## Status

Accepted.

## Context

The compatibility Redis key store persists virtual-key `allowed_models` and
SCIM/admin `allowed_key_prefixes` as JSON array hash fields. Loader helpers
previously converted malformed JSON or whitespace-bearing entries into an empty
list.

Affected symbols:

- `runtime_gateway_redis_stored_key_from_hash`
- `runtime_gateway_redis_scim_user_from_hash`
- `runtime_gateway_redis_hash_exact_json_vec`

The risk is fail-open authorization: an empty `allowed_models` or key-prefix
list can mean unrestricted compatibility behavior, so corrupted persisted Redis
arrays must not silently become empty policy.

## Decision

Missing or empty legacy Redis JSON array fields still load as empty, but
present values must be valid JSON string arrays with non-empty,
whitespace-free entries. Malformed present values now fail Redis key-store
loading.

## Consequences

Corrupt Redis compatibility key-store arrays are visible during load and cannot
silently weaken model or admin key-prefix restrictions. Legacy records that
omit the fields retain the previous empty-list behavior.

Regression coverage:

- `redis_exact_json_vec_rejects_whitespace_entries`
- `redis_key_store_hash_rejects_malformed_json_array_fields`
