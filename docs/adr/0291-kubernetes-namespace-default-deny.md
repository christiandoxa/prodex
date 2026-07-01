# ADR 0291: Kubernetes Namespace Default Deny

## Status

Accepted.

## Context

The Kubernetes baseline defines explicit NetworkPolicies for the gateway,
migration Job, and placeholder control plane. Those policies protect known
workloads, but a future pod added to the `prodex` namespace without a matching
workload-specific policy could otherwise start with implicit network access.

## Decision

Add a namespace-wide `prodex-default-deny` NetworkPolicy with `podSelector: {}`
and `policyTypes: ["Ingress", "Egress"]`.

The policy has no ingress or egress allow rules. Workload-specific
NetworkPolicies remain responsible for granting only DNS, data-store, ingress,
and gateway provider egress paths where needed.

## Consequences

- New pods in the `prodex` namespace are isolated by default.
- Gateway, migration, and control-plane NetworkPolicies become explicit allow
  lists layered on top of the namespace deny baseline.
- The deployment security guard now rejects missing, non-global, ingress-only,
  egress-only, or rule-bearing default-deny policies.
