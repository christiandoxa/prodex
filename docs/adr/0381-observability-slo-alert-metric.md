# ADR 0381: Observability SLO Alert Metric

## Status

Accepted.

## Context

Enterprise operations require minimum SLO alert coverage for availability, p95 latency, error rate, quota correctness, provider degradation, and persistence failure. Alert decisions contain objective names, observed values, targets, tenant context, request context, and routing details that must not become high-cardinality metric labels.

## Decision

Add `plan_slo_alert_metric` to `prodex-observability`.

The planner emits `prodex_slo_alert_events_total`, increments by one, and uses only the closed enum labels `slo_sli` and `slo_severity`.

## Consequences

- Gateway, control-plane, and operational adapters can publish SLO alert events through a shared low-cardinality contract.
- Objective names, observed values, targets, tenants, request IDs, and alert routing details remain trace/log/report data subject to redaction.
- The observability boundary guard can enforce the SLO alert telemetry contract before a concrete metrics backend is wired in.
