# ADR 1041: SQL key-store JSON array fail-closed parsing

## Status

Accepted.

## Context

SQLite and PostgreSQL compatibility key stores persist virtual-key
`allowed_models` and SCIM/admin `allowed_key_prefixes` as JSON string arrays.
The shared loader helper previously converted malformed JSON or
whitespace-bearing entries into an empty list.

Affected symbols:

- `runtime_gateway_sqlite_load_key_store_from_conn`
- `runtime_gateway_sqlite_load_scim_users_from_conn`
- `runtime_gateway_postgres_load_scim_users_from_client`
- `runtime_gateway_postgres_stored_key_from_row`
- `runtime_gateway_exact_json_vec_for_field`

The risk is fail-open policy: an empty allow-list can mean unrestricted
compatibility behavior, so corrupt SQL-backed policy arrays must not silently
become empty.

## Decision

SQL-backed key-store JSON arrays must be valid JSON string arrays with
non-empty, whitespace-free entries. Malformed present values now fail the
key-store load instead of becoming empty lists.

## Consequences

Corrupt SQL-backed compatibility policy is visible during load and cannot
silently weaken model or admin key-prefix restrictions.

Regression coverage:

- `exact_json_vec_rejects_whitespace_entries_across_backends`
- `sqlite_key_store_rejects_malformed_allowed_models_json`
