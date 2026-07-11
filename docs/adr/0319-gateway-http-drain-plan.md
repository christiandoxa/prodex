# ADR 0319: Gateway HTTP Drain Plan

## Status

Accepted.

## Context

The gateway HTTP policy already carries a connection-drain timeout and the
Kubernetes baseline includes a `preStop` delay plus termination grace. Those
values need a shared boundary check so deployment adapters do not configure a
termination window shorter than the time needed for endpoint removal and active
connection draining.

## Decision

Add `GatewayHttpDrainPlan`, `GatewayHttpDrainPlanError`, and
`plan_gateway_http_drain` to `prodex-gateway-http`.

The planner validates the HTTP policy, adds the `preStop` delay to
`connection_drain_timeout_ms`, rejects missing `preStop` delay with
`PreStopDelayRequired`, rejects shorter termination grace with
`TerminationGraceTooShort`, and records that readiness must fail before drain.

`ci:gateway-http-boundary-guard` now checks that the drain plan, readiness
failure marker, non-zero preStop requirement, preStop-inclusive grace
calculation, and short-grace rejection remain present.

## Consequences

Async HTTP and deployment composition roots can validate graceful shutdown
timing without importing Kubernetes, Axum, Hyper, Tower, or Tokio into the
boundary crate. Rolling updates have an explicit framework-neutral contract
linking readiness failure, preStop delay, connection drain, and termination
grace.
