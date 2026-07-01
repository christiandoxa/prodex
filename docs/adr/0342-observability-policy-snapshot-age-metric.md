# ADR 0342: Observability Policy Snapshot Age Metric

## Status

Accepted.

## Context

Enterprise gateways need policy age telemetry so operators can detect stale,
expired, or invalidated policy snapshots. This metric must not expose tenant
IDs, active or last-known-good revision IDs, policy digests, signatures, request
IDs, or raw policy payload details as labels.

## Decision

Add `plan_policy_snapshot_age_metric` to `prodex-observability`.

The planner evaluates `PolicyCacheStatus` with `evaluate_policy_refresh`, emits
`prodex_policy_snapshot_age_ms`, stores snapshot age as a measurement value, and
exposes only the low-cardinality `policy_cache_state` label.

## Consequences

- Gateway adapters can publish policy age telemetry without duplicating refresh
  decision semantics.
- Missing snapshots still produce a state label but no age measurement.
- Revision IDs, tenant IDs, digests, signatures, and policy payload details
  remain trace/log concerns subject to redaction policy, not metric labels.
