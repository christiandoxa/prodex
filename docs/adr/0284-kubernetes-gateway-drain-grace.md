# ADR 0284: Kubernetes Gateway Drain Grace

## Status

Accepted.

## Context

The gateway HTTP boundary models graceful drain and streaming backpressure, and
the Kubernetes manifest exposes readiness/liveness/startup probes. During a
rolling update or eviction, Kubernetes can still send SIGTERM shortly after
removing an endpoint. Without an explicit grace window, streaming requests and
provider responses have less time to drain cleanly.

## Decision

Add a gateway `terminationGracePeriodSeconds: 45` and a `preStop` hook that
sleeps for 15 seconds. The delay gives endpoint removal and load balancers time
to stop routing new traffic before process termination, while the grace period
leaves room for the application drain budget.

The deployment security guard now requires both markers and rejects a manifest
without the gateway termination grace period.

## Consequences

- Rolling updates and node drains are less likely to interrupt streaming
  gateway requests immediately.
- The manifest remains a baseline; future async gateway wiring should also make
  `/readyz` false while the process is draining.
- Operators can tune the durations, but removing drain grace must be deliberate
  and will fail the repository guard.
