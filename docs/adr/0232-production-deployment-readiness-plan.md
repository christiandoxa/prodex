# 0232: Production Deployment Readiness Plan

## Status

Accepted.

## Context

Kubernetes manifests can look production-shaped while still being unsafe for
multi-replica accounting. A production deployment must declare at least two
gateway replicas, enable the multi-replica accounting gate, and use shared
PostgreSQL and Redis dependencies before it can claim horizontal data-plane
readiness.

## Decision

Add `ProductionReadinessTopology` and `plan_production_deployment_readiness` to
`prodex-domain`.

The planner accepts production readiness only when:

- gateway replica count is at least two;
- multi-replica accounting checks are required;
- shared PostgreSQL is configured; and
- shared Redis is configured.

Failures use the same redacted deployment security response boundary as other
deployment validation failures.

## Consequences

Deployment manifests, runtime startup checks, and control-plane readiness APIs
can use one domain contract for production topology validation. Trusted
diagnostics can inspect the exact violations, while public readiness responses
stay stable and generic.
