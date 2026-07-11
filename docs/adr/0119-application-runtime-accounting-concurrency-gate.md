# ADR 0119: Application runtime accounting concurrency gate

## Status

Accepted

## Context

The storage boundary defines the required multi-replica accounting concurrency
checks, but runtime composition still needs a way to require those checks when a
deployment is intended to be enterprise-ready. Without an application-level
gate, composition roots could configure a single replica, SQLite, or no Redis
while still claiming multi-replica accounting readiness.

## Decision

Extend `ApplicationRuntimeTopology` with a gateway replica count and an explicit
`require_multi_replica_accounting_checks` flag. When enabled,
`plan_application_runtime` delegates to
`plan_multi_replica_accounting_concurrency_spec` and returns the resulting spec
in `ApplicationRuntimePlan`.

Invalid topologies are rejected before any adapter is started. Valid
enterprise accounting topologies must use PostgreSQL, Redis, and at least two
gateway replicas.

## Consequences

Deployment composition can opt in to a clear production-readiness gate for
accounting concurrency. The gate remains side-effect-free and does not run the
database tests itself; it selects the invariant set that external integration
tests and deployment pipelines must execute.
