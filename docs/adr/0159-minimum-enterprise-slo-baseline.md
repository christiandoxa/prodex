# ADR 0159: Minimum enterprise SLO baseline

## Status

Accepted

## Context

The enterprise objective requires SLI/SLO and alert coverage for availability,
latency, error rate, quota correctness, provider degradation, and persistence
failure. `prodex-domain` already models SLI kinds, objectives, measurements, and
alert decisions, but composition roots need one shared minimum baseline so
gateway, control-plane, and operational adapters do not define incompatible
alert defaults.

## Decision

Add `minimum_enterprise_slo_objectives` to `prodex-domain`. The function returns
the side-effect-free minimum SLO baseline for:

- availability;
- p95 latency;
- error rate;
- quota correctness;
- provider degradation; and
- persistence failures.

The baseline remains adapter-neutral and contains no metric backend, runtime, or
HTTP dependencies.

## Consequences

Composition roots can opt into a consistent enterprise SLO baseline while still
allowing production policy to override thresholds. The domain boundary now
captures the objective's required minimum operational signals in code and tests.
