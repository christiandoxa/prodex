# ADR 1061: Legacy Gateway Signal Drain

## Status

Accepted.

## Context

The legacy `prodex gateway` process parked forever after starting its bounded
worker pool. SIGTERM therefore used the operating-system default termination,
without first changing readiness or waiting for admitted requests. At the time
of this decision the async dedicated serve adapter was not wired, although
Kubernetes already declared a termination grace period and preStop delay.

## Decision

Register SIGTERM and SIGINT through Tokio's signal driver after gateway startup.
On receipt, set the existing shared shutdown/draining flag, unblock accept
workers, and wait for the existing atomic active-request count to reach zero.
Use the production gateway HTTP connection-drain timeout of 30 seconds. Log only
structured drain start, completion, timeout, and aggregate active-request count
markers. A timeout returns a non-zero process result.

## Consequences

- `/readyz` becomes unavailable through the existing draining check before the
  wait begins; `/livez` and `/startupz` keep their established semantics.
- No new request-path lock, thread, filesystem read, or network operation is
  introduced.
- In-flight streams get a bounded completion window; shutdown never waits
  forever.
- Worker joins and connection draining remain compatibility behavior. The
  dedicated async server must own accept cancellation and native graceful
  connection shutdown before the legacy server can be removed.
