# ADR 0480: Gateway open paths do not ALTER TABLE

## Status

Accepted.

## Context

Gateway SQLite/PostgreSQL open helpers still executed compatibility
`ALTER TABLE` statements. These helpers are reused by request, admin, usage, and
reconciliation paths, so schema mutation could still happen from request-serving
code.

## Decision

Remove compatibility `ALTER TABLE` execution from gateway SQL open helpers.
Fresh stores still bootstrap the current schema with complete `CREATE TABLE IF
NOT EXISTS` definitions. Existing older stores require an explicit migration
path instead of request-path mutation.

## Consequences

Gateway SQL open paths no longer run expand DDL. Full versioned migration
commands and startup compatibility checks remain future work.
