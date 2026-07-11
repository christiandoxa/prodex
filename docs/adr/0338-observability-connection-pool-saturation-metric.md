# ADR 0338: Observability Connection Pool Saturation Metric

## Status

Accepted.

## Context

Enterprise operations require connection pool saturation telemetry for storage,
cache, provider HTTP, and OIDC HTTP clients. Pool telemetry must not turn tenant
IDs, request IDs, hostnames, URLs, credential names, or per-endpoint details into
metric labels.

## Decision

Add `plan_connection_pool_saturation_metric` to `prodex-observability`.

The planner emits the gauge name `prodex_connection_pool_in_use`, keeps `in_use`
and `capacity` as measurement fields, and exposes only the closed enum metric
label `pool_kind` derived from `ConnectionPoolKind`.

## Consequences

- Adapters get one shared connection pool saturation metric contract.
- Pool labels stay low-cardinality across tenants and deployments.
- Backend endpoints, credential names, pool capacities, and per-request details
  remain values or redacted trace/log fields, not metric labels.
