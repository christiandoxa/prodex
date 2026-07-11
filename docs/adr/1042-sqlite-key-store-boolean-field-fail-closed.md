# ADR 1042: SQLite key-store boolean field fail-closed parsing

## Status

Accepted.

## Context

The SQLite compatibility key store persists boolean governance state as integer
fields, including virtual-key `disabled` and SCIM user `active`. Loaders
previously treated every non-zero integer as `true`.

Affected symbols:

- `runtime_gateway_sqlite_load_key_store_from_conn`
- `runtime_gateway_sqlite_load_scim_users_from_conn`
- `runtime_gateway_sqlite_exact_bool`

The risk is fail-open or ambiguous governance: malformed integer values can
change whether a key is disabled or a SCIM user is active.

## Decision

SQLite key-store boolean fields must be exact `0` or `1`. Any other present
integer now fails key-store loading instead of being normalized through
non-zero truthiness.

## Consequences

Corrupt SQLite compatibility key-store records are visible during load and
cannot silently change authorization state.

Regression coverage:

- `sqlite_key_store_rejects_malformed_boolean_fields`
