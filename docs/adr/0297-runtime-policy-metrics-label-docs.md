# ADR 0297: Runtime Policy Metrics Label Documentation Guard

## Status

Accepted

## Context

ADR 0009 and the gateway metrics implementation intentionally avoid raw tenant,
team, project, user, budget, and virtual-key names in Prometheus labels. The
runtime policy reference described scoped metrics as if those raw governance
dimensions were exported directly, which contradicted the implemented
`key_hash` plus scope-boolean contract and could lead operators to build
high-cardinality or sensitive dashboards.

## Decision

The runtime policy reference documents virtual-key Prometheus series as using a
stable `key_hash` and low-cardinality booleans such as `tenant_scoped` and
`budget_scoped`. It explicitly states that raw tenant, team, project, user,
budget, and virtual-key names are not metric labels.

The runtime policy documentation check rejects the old raw-label wording and
requires the bounded-label contract phrases to remain present.

## Consequences

- Documentation now matches the runtime metric implementation and ADR 0009.
- Future docs updates cannot silently reintroduce raw governance identifiers as
  advertised Prometheus labels.
- Operators must join raw tenant or user metadata outside Prometheus label sets
  when they need rich reporting.
