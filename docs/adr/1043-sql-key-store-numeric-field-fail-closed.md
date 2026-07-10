# ADR 1043: SQL key-store numeric field fail-closed parsing

## Status

Accepted.

## Context

SQLite and PostgreSQL compatibility key stores persist virtual-key budgets,
rate limits, and timestamps as signed SQL integers. Loaders previously reused
loss-tolerant conversion helpers that mapped negative values to `0`.

Affected symbols:

- `runtime_gateway_sqlite_load_key_store_from_conn`
- `runtime_gateway_sqlite_load_scim_users_from_conn`
- `runtime_gateway_postgres_load_scim_users_from_client`
- `runtime_gateway_postgres_stored_key_from_row`
- `runtime_gateway_sqlite_key_store_u64`
- `runtime_gateway_sql_key_store_i64_to_u64`

The risk is policy/accounting drift: corrupt negative budgets, limits, or
timestamps can be normalized into weaker or misleading runtime state.

## Decision

SQL-backed key-store budget, rate-limit, and timestamp fields must be
non-negative when present. Negative values now fail key-store loading instead
of silently becoming `0`.

## Consequences

Corrupt SQL-backed compatibility key-store records are visible during load and
cannot silently change budget, rate-limit, or timestamp semantics.

Regression coverage:

- `sqlite_key_store_rejects_negative_numeric_fields`
- `postgres_key_store_rejects_negative_numeric_fields`
