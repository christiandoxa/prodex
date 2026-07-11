# ADR 0978: Gateway compatibility schema uses versioned migrations

## Status

Accepted

## Context

Gateway request-path opens no longer run compatibility DDL, and
`prodex-gateway migrate` already owns that work. The remaining weakness was the
compatibility schema implementation itself: it still materialized one
current-state SQL blob instead of an explicit ordered migration list.

That made future compatibility changes harder to audit and easy to grow
outside the same expand/backfill/contract discipline already used by the main
storage migration plans.

## Decision

Move the legacy gateway compatibility schema onto explicit versioned migration
lists for SQLite and PostgreSQL.

- The external migrator path now ensures the schema-migrations table, applies
  each compatibility migration in version order, and records the applied
  version.
- The first compatibility migration preserves the historical base ledger shape;
  the next migration upgrades legacy billing-ledger tables in place so old
  deployments gain typed `request_id` plus tenant/governance scope snapshot
  columns without reopening request-path DDL.
- Request-serving gateway opens still require an already-migrated schema and no
  longer carry a bootstrap flag.
- Regression coverage verifies the SQLite compatibility migrator is idempotent
  and records the expected version history, including upgrade from the legacy
  ledger column set.

## Consequences

- Compatibility schema growth now has a real place for expand/backfill/contract
  steps instead of rewriting one monolithic bootstrap block.
- Request-serving opens stay DDL-free.
- Future destructive compatibility changes still need deliberate contract
  choreography, but the versioned migration boundary now exists for them.
