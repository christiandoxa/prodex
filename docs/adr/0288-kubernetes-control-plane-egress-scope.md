# ADR 0288: Kubernetes Control-Plane Egress Scope

## Status

Accepted.

## Context

The Kubernetes baseline separates data-plane gateway pods from the placeholder
control-plane deployment. Gateway pods need public provider HTTPS egress for
inference traffic. The control plane manages tenants, identity, policy, billing,
and configuration; it should not inherit provider egress by default, especially
after its secret scope no longer includes provider API keys.

## Decision

Remove broad public HTTPS egress from the `prodex-control-plane` NetworkPolicy.
The placeholder control plane may egress to DNS plus data-store namespaces for
PostgreSQL and Redis, but not to public provider endpoints.

The deployment security guard now inspects the control-plane NetworkPolicy and
fails if it contains `0.0.0.0/0` or port `443` provider egress.

## Consequences

- Data-plane provider access remains scoped to gateway pods.
- Control-plane rollout starts from a narrower network posture and can add
  explicit outbound integrations later.
- Operators that need IdP or webhook egress for control-plane adapters must add
  explicit destinations deliberately.
