# ADR 0356: Observability Secret Rotation Metric

## Status

Accepted.

## Context

Enterprise deployments need telemetry for secret rotation across provider
credentials, OIDC clients, signing keys, storage credentials, and webhook
secrets. These metrics must not label by tenant ID, secret ID, filesystem path,
provider endpoint, credential value, key material, request ID, or raw secret
provider error text.

## Decision

Add `plan_secret_rotation_metric` to `prodex-observability`.

The planner emits `prodex_secret_rotation_events_total`, increments by one, and
exposes only the closed enum labels `secret_scope` and
`secret_rotation_result`.

## Consequences

- Secret provider adapters can publish rotation success, failure, skip, and
  rollback counters through one shared contract.
- Secret rotation labels remain low-cardinality and reviewable.
- Tenant IDs, secret IDs, paths, endpoints, credential values, key material,
  request IDs, and raw provider errors remain trace/log/audit concerns subject
  to redaction policy.
