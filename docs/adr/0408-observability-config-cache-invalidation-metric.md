# ADR 0408: Configuration cache invalidation metric

## Status

Accepted

## Context

Revisioned configuration and policy caches need explicit invalidation and
last-known-good behavior across gateway policy caches, runtime policy caches,
and Redis-backed short-lived policy revision caches. Operators need aggregate
telemetry for invalidated, reload-scheduled, not-found, and failed invalidation
outcomes without exposing tenant IDs, revision IDs, root paths, cache keys,
payloads, caller identity, or raw invalidation errors in metric labels.

## Decision

Add `plan_config_cache_invalidation_metric` to `prodex-observability`.

The planner emits `prodex_config_cache_invalidation_events_total`, increments
by one, and uses only the closed enum labels `config_invalidation_target` and
`config_invalidation_result`.

## Consequences

- Gateway, runtime-policy, and Redis cache invalidation adapters can publish
  counters through one low-cardinality boundary.
- The observability boundary guard can enforce the metric name and label keys.
- Tenant IDs, revision IDs, root paths, cache keys, payloads, caller identity,
  and raw invalidation errors must stay out of metric labels.
