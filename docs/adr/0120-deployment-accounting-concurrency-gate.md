# ADR 0120: Deployment accounting concurrency gate

## Status

Accepted

## Context

The application runtime can require a multi-replica accounting concurrency spec,
but deployment artifacts also need to expose the intent clearly. A Kubernetes
baseline that scales gateway replicas without declaring the accounting gate and
shared Redis/PostgreSQL dependencies could be misread as production-ready while
not satisfying the enterprise accounting checks.

## Decision

Add deployment-level configuration for the accounting concurrency gate:

- `PRODEX_GATEWAY_REPLICA_COUNT` declares the intended gateway replica count;
- `PRODEX_REQUIRE_MULTI_REPLICA_ACCOUNTING_CHECKS` enables the runtime gate; and
- `PRODEX_GATEWAY_REDIS_URL` is provided through ExternalSecret alongside the
  PostgreSQL URL.

These topology environment values are exact runtime inputs. Replica count and
gate booleans must be non-empty and whitespace-free; see
`docs/adr/1032-gateway-accounting-topology-env-exact-boundary.md`.

The Kubernetes baseline enables the gate with three gateway replicas. The local
Compose environment example keeps the gate disabled by default and documents
that enabling it requires shared PostgreSQL, Redis, and at least two replicas.

## Consequences

Production-shaped manifests now align with the application runtime gate and the
storage concurrency spec. Local/single-node examples remain usable without
claiming multi-replica accounting readiness.
