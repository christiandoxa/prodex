# 0624. SLO Negative Value Guard

## Status

Accepted

## Context

Enterprise SLO inputs model availability percentages, latency, error rates,
quota correctness, provider degradation, and persistence failure counts. These
signals are non-negative by definition.

The domain evaluator already ignored non-finite values, but negative observed or
target values could still participate in alert decisions.

## Decision

`evaluate_slo` now ignores SLO evaluations when the observed measurement or
objective target is negative.

## Consequences

- Invalid telemetry cannot produce misleading degraded/unavailable responses.
- Public SLO response planning remains unchanged and redacted.
- Adapters should still validate data-source-specific SLI units before creating
  measurements.
