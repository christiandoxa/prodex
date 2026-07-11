# ADR 0337: Observability Bounded Queue Depth Metric

## Status

Accepted.

## Context

Enterprise operations require bounded queue depth telemetry for gateway lanes,
telemetry export, and persistence work. Queue depth must be observable without
turning tenant IDs, request IDs, queue names from runtime configuration, or raw
capacity values into metric labels.

## Decision

Add `plan_queue_depth_metric` to `prodex-observability`.

The planner emits the gauge name `prodex_queue_depth`, keeps `depth` and
`capacity` as measurement fields, and exposes only the closed enum metric label
`queue_kind` derived from `QueueDepthKind`.

## Consequences

- Adapters can publish bounded queue depth without inventing local label names.
- Queue kinds stay low-cardinality and reviewable.
- Runtime capacities and per-request details remain values or trace/log fields,
  not metric labels.
