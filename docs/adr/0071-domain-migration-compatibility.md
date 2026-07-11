# ADR 0071: Domain migration compatibility and expand/contract order

## Status

Accepted.

## Context

The enterprise database migration target requires versioned migrations to run
outside request-serving paths, with locking, startup compatibility checks,
forward-compatible expand/contract procedure, and migration tests from at least
two previous versions. `prodex-domain` already modeled dedicated migrator versus
request-serving execution, lock ownership, and failed-step blocking, but it did
not encode expand/backfill/verify/contract ordering or the minimum compatibility
window.

## Decision

Add `MigrationStepKind`, `validate_expand_contract_order`,
`MigrationCompatibilityWindow`, and `validate_migration_compatibility`. Migration
plans now reject backfill before expand, verify before backfill, and contract
before expand. Compatibility windows must list at least two previous source
versions and can reject unsupported upgrade sources before a migrator runs.

## Consequences

- Future gateway startup compatibility checks and migrator commands can share the
  same pure decision model.
- Request-serving execution remains forbidden by the existing plan gate.
- Phase 2 storage work still needs concrete versioned migration files, database
  locks, and integration tests; this ADR defines the domain semantics those
  implementations must use.
