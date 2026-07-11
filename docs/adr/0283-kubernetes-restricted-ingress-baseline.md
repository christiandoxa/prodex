# ADR 0283: Kubernetes Restricted Ingress Baseline

## Status

Accepted.

## Context

The Kubernetes NetworkPolicy baseline already restricts egress to DNS,
data-store namespaces, and public provider HTTPS with private-network
exceptions. Ingress still accepted traffic from any namespace through
`namespaceSelector: {}`. In regulated multi-tenant clusters, that allows
unrelated workloads in the cluster to reach gateway or control-plane services
unless separate cluster policy blocks them.

## Decision

Restrict gateway and placeholder control-plane NetworkPolicy ingress to
namespaces labeled `prodex.dev/network-tier=ingress`. The deployment security
guard now requires the ingress label marker and rejects manifests that allow
all namespaces through an empty ingress namespace selector.

## Consequences

- The production-shaped baseline requires an explicit ingress/admin namespace
  trust boundary.
- Operators must label their ingress controller namespace or replace the
  selector with their private access policy.
- Removing the ingress restriction becomes a deliberate manifest change that
  fails the repository guard.
