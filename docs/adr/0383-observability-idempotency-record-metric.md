# ADR 0383: Observability Idempotency Record Metric

## Status

Accepted.

## Context

Enterprise control-plane mutations require durable idempotency records so retries and replays do not apply mutations twice. Operators need to count pending inserts, completions, and lookup outcomes without exposing raw idempotency keys, request fingerprints, tenant IDs, request details, or stored response bodies as metric labels.

## Decision

Add `plan_idempotency_record_metric` to `prodex-observability`.

The planner emits `prodex_idempotency_record_events_total`, increments by one, and uses only the closed enum labels `idempotency_record_backend`, `idempotency_record_operation`, and `idempotency_record_result`.

## Consequences

- PostgreSQL and SQLite idempotency adapters can publish durable replay record outcomes through a shared low-cardinality contract.
- Idempotency keys, request fingerprints, tenants, request details, and stored response bodies remain trace/log/report data subject to redaction.
- The observability boundary guard can enforce the idempotency record telemetry contract before a concrete metrics backend is wired in.
