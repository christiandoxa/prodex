# ADR 0361: Observability Migration Lifecycle Metric

## Status

Accepted.

## Context

Enterprise deployments need migration telemetry for status checks,
compatibility checks, apply operations, and rollbacks. These metrics must not
label by tenant ID, migration version, lock owner, database endpoint, schema
name, request ID, or raw migration error text.

## Decision

Add `plan_migration_lifecycle_metric` to `prodex-observability`.

The planner emits `prodex_migration_lifecycle_events_total`, increments by one,
and exposes only the closed enum labels `migration_operation` and
`migration_result`.

## Consequences

- Migration jobs and startup compatibility checks can publish lifecycle events
  through one shared contract.
- Migration labels remain low-cardinality and reviewable.
- Tenant IDs, versions, lock owners, endpoints, schema names, request IDs, and
  raw migration errors remain trace/log/report concerns subject to redaction
  policy.
