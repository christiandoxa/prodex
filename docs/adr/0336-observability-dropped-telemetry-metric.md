# ADR 0336: Observability Dropped Telemetry Metric

## Status

Accepted.

## Context

Enterprise operations need an explicit dropped telemetry count so alerting can
detect exporter outages, bounded queue pressure, shutdown drops, and invalid
payload drops. The metric must not expose tenant, user, request, prompt, or raw
error details as labels.

## Decision

Add `plan_dropped_telemetry_metric` to `prodex-observability`.

The planner emits the counter name `prodex_telemetry_dropped_total`, increments
by one, and exposes only the closed enum label `telemetry_drop_reason` with
values derived from `TelemetryDropReason`.

## Consequences

- Adapters get one shared dropped telemetry counter contract.
- Drop reasons stay low-cardinality and reviewable.
- Raw exporter errors and tenant/request details remain trace/log concerns
  subject to redaction policy, not metric labels.
