# ADR 0403: PostgreSQL tenant context metric

## Status

Accepted

## Context

PostgreSQL Row-Level Security depends on request-serving adapters setting the
tenant session context before tenant-owned DML. Operators need aggregate
telemetry for context setup, context verification, RLS policy enforcement,
tenant DML execution, missing context, mismatches, denials, and failures without
exposing tenant IDs, statement text, query text, database endpoints, storage
keys, or raw storage errors in metric labels.

## Decision

Add `plan_postgres_tenant_context_metric` to `prodex-observability`.

The planner emits `prodex_postgres_tenant_context_events_total`, increments by
one, and uses only the closed enum labels `postgres_tenant_context_operation`
and `postgres_tenant_context_result`.

## Consequences

- PostgreSQL storage adapters can publish RLS tenant-context counters through
  one low-cardinality boundary.
- The observability boundary guard can enforce the metric name and label keys.
- Tenant IDs, SQL statements, query text, database endpoints, storage keys,
  request payloads, and raw storage errors must stay out of metric labels.
