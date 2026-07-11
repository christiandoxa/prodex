# 0169: SLO Alert Stable Responses

## Status

Accepted

## Context

Enterprise operations require minimum SLO coverage for availability, latency,
error rate, quota correctness, provider degradation, and persistence failures.
Raw alert decisions include objective names and observed/target values. Those
fields are useful for trusted telemetry and alert routing, but they can expose
tenant-specific objective names, operational thresholds, and high-cardinality
measurements if returned by public readiness or control-plane APIs.

## Decision

`prodex-domain` owns `plan_slo_alert_response`.

The planner maps an `AlertDecision` to a stable status/code/message response
plan. Warning alerts become degraded responses; critical alerts become
unavailable responses. The response code identifies only the SLI class. The
planner deliberately omits objective names, observed values, targets, tenant
names, and metric-label details.

## Consequences

- Public operational endpoints can report degraded/unavailable state without
  leaking threshold internals.
- Alert routing can keep raw `AlertDecision` data in trusted telemetry.
- Enterprise SLO behavior remains domain-pure and reusable across gateway and
  control-plane composition roots.
