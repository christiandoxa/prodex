# ADR 0404: Redis coordination metric

## Status

Accepted

## Context

Redis is allowed only as a distributed rate limiter, short-lived cache, or
rebuildable coordination primitive. Operators need aggregate telemetry for
limiter checks and commits, recovery lease acquire/release, cache reads/writes,
limited decisions, cache misses, unavailability, and failures without exposing
tenant IDs, Redis keys, lease-owner tokens, endpoints, script SHAs, request
payloads, or raw storage errors in metric labels.

## Decision

Add `plan_redis_coordination_metric` to `prodex-observability`.

The planner emits `prodex_redis_coordination_events_total`, increments by one,
and uses only the closed enum labels `redis_coordination_operation` and
`redis_coordination_result`.

## Consequences

- Redis adapters can publish limiter, cache, and coordination counters through
  one low-cardinality boundary.
- The observability boundary guard can enforce the metric name and label keys.
- Tenant IDs, Redis keys, lease-owner tokens, endpoints, script SHAs, request
  payloads, and raw storage errors must stay out of metric labels.
