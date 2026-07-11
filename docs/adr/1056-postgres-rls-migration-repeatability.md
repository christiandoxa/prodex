# ADR 1056: PostgreSQL RLS Migration Repeatability

## Status

Accepted.

## Context

`prodex-gateway migrate` applies every declared enterprise PostgreSQL migration.
The initial migration used `CREATE TABLE IF NOT EXISTS` but unconditional
`CREATE POLICY` statements, so a second invocation failed after the tables and
policies already existed. Operators must be able to retry a failed or uncertain
migration command without first deleting tenant isolation policy.

## Decision

Create the fixed set of tenant-isolation policies from one PostgreSQL `DO`
block. The block checks `pg_policies` in the current schema and creates only a
missing policy. Existing policies are left in place; policy changes require a
new versioned migration rather than mutating migration 1.

The PostgreSQL execution test applies migration 1 twice against the same clean
database and verifies that exactly one isolation policy exists for each of the
twelve tenant-owned tables.

## Consequences

- Retrying the external migrator no longer fails on duplicate RLS policies.
- A retry never creates a temporary policy-free window.
- Migration 1 remains compatible with databases where only some policies were
  created before an earlier failure.
- Destructive policy changes remain explicit roll-forward migrations.
