# ADR 0121: Kubernetes restricted egress baseline

## Status

Accepted

## Context

The production-shaped Kubernetes manifest includes NetworkPolicy resources, but
regulated deployments need more than the presence of a policy object. Allowing
all in-cluster namespaces or all private-network egress would weaken tenant and
control-plane isolation and make it harder to reason about PostgreSQL/Redis as
the only shared accounting backends.

## Decision

Restrict gateway and control-plane egress in the baseline manifest to:

- DNS on TCP/UDP 53 to `kube-dns`;
- PostgreSQL on TCP 5432 and Redis on TCP 6379 in namespaces labeled
  `prodex.dev/network-tier=data-store`; and
- HTTPS on TCP 443 to public provider endpoints, excluding RFC1918 private
  address ranges from that broad egress rule.

The deployment security guard now checks for these selectors, ports, and private
range exclusions, and rejects unbounded `namespaceSelector: {}` egress patterns.

## Consequences

Operators must explicitly label or replace their private data-store namespace
selectors when applying the baseline. This keeps the repository artifact aligned
with regulated-environment expectations while still allowing public provider
HTTPS egress by default.
