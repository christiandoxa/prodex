# ADR 0358: Observability Deployment Rollout Metric

## Status

Accepted.

## Context

Enterprise deployments need rollout telemetry for applying, verifying,
promoting, and rolling back production artifacts. These metrics must not label
by Kubernetes namespace, pod name, image digest, rollout revision, tenant ID,
environment name, request ID, or raw deployment error text.

## Decision

Add `plan_deployment_rollout_metric` to `prodex-observability`.

The planner emits `prodex_deployment_rollout_events_total`, increments by one,
and exposes only the closed enum labels `deployment_rollout_operation` and
`deployment_rollout_result`.

## Consequences

- Deployment and control-plane adapters can publish rollout apply, verify,
  promote, and rollback counters through one shared contract.
- Rollout labels remain low-cardinality and reviewable.
- Namespaces, pod names, image digests, revisions, tenants, environments,
  request IDs, and raw deployment errors remain trace/log/audit concerns subject
  to redaction policy.
