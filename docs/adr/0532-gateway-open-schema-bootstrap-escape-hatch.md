# ADR 0532: Gate legacy gateway schema bootstrap

## Status

Accepted

## Context

The enterprise migration target requires request-serving gateway paths to avoid
schema migration DDL. The legacy local rewrite gateway SQL open helper still
bootstrapped fresh SQLite/PostgreSQL schemas with `CREATE TABLE IF NOT EXISTS`,
which made gateway startup/request paths act as implicit migrators.

## Decision

Make legacy schema bootstrap an explicit compatibility escape hatch through
`PRODEX_GATEWAY_ALLOW_REQUEST_PATH_SCHEMA_BOOTSTRAP`. Normal production opens
verify the existing `prodex_gateway_schema_migrations` version and fail closed
when the schema has not been migrated.

Unit tests may call the inner helper with explicit bootstrap permission to keep
local compatibility coverage cheap while still testing the production-deny path.

## Consequences

Gateway SQL open paths stop silently creating schemas in production. Fresh SQL
state now requires an explicit migrator or a deliberate local/test bootstrap
escape hatch until the full migration command is wired end to end.
