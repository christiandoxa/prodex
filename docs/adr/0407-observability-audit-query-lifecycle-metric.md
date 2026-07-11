# ADR 0407: Audit query lifecycle metric

## Status

Accepted

## Context

Regulated control planes need audit query and export telemetry for query
planning, paginated reads, export planning, export serialization, empty results,
denials, and failures. These counters must not expose tenant IDs, principal
identifiers, audit event IDs, cursors, time ranges, export formats, request
payloads, or raw storage errors in metric labels.

## Decision

Add `plan_audit_query_lifecycle_metric` to `prodex-observability`.

The planner emits `prodex_audit_query_lifecycle_events_total`, increments by
one, and uses only the closed enum labels `audit_query_operation` and
`audit_query_result`.

## Consequences

- Audit query and export adapters can publish lifecycle counters through one
  low-cardinality boundary.
- The observability boundary guard can enforce the metric name and label keys.
- Tenant IDs, principal IDs, audit event IDs, cursors, time ranges, export
  formats, request payloads, and raw storage errors must stay out of metric
  labels.
