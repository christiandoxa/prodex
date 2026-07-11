# ADR 0354: Observability Health Probe Metric

## Status

Accepted.

## Context

Enterprise gateways need telemetry for `/livez`, `/readyz`, and `/startupz`
probe results, including degraded and draining states. These metrics must not
label by raw path, tenant ID, dependency name, backend endpoint, policy
revision, pod name, request ID, or raw dependency error text.

## Decision

Add `plan_health_probe_metric` to `prodex-observability`.

The planner emits `prodex_health_probe_results_total`, increments by one, and
exposes only the closed enum labels `health_probe` and `health_result`.

## Consequences

- Gateway and control-plane adapters can publish health probe outcomes through
  one shared contract.
- Probe labels remain low-cardinality and reviewable.
- Raw paths, tenant IDs, dependency names, backend endpoints, policy revisions,
  pod names, request IDs, and raw dependency errors remain trace/log/audit
  concerns subject to redaction policy.
