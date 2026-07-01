# ADR 0160: Health probe stable responses

## Status

Accepted

## Context

The enterprise target requires `/livez`, `/readyz`, `/startupz`, graceful
draining, and readiness false while a replica is draining or lacks an active
policy revision. `prodex-domain` already models the health snapshot and probe
state decisions, but composition roots still need a stable redacted response
plan so operational probes do not leak dependency messages, active policy
revision identifiers, backend endpoints, or drain internals.

## Decision

Add `HealthProbeKind`, `HealthProbeResponsePlan`, and
`plan_health_probe_response` to `prodex-domain`. The planner maps a
`HealthSnapshot` plus probe kind to a stable probe/state/ready/code/message
tuple for `livez`, `readyz`, and `startupz`.

The response messages remain generic and exclude health-check names, raw
health-check messages, active policy revision IDs, backend endpoints, and other
diagnostic internals.

## Consequences

Gateway and control-plane adapters can expose deterministic public health probe
responses while retaining detailed health checks for trusted diagnostics and
logs. Readiness remains false during draining and when startup or policy
activation is incomplete.
