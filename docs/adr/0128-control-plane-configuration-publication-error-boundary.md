# ADR 0128: Control-plane configuration publication error boundary

## Status

Accepted

## Context

Configuration publication is a composed control-plane use case. It can fail
before publication because the principal is not authorized, or after
authorization because the candidate revision violates the configuration boundary.
Adapters need one stable, HTTP-neutral response plan for the whole use case
instead of duplicating mapping logic across admin API handlers.

## Decision

Add `plan_configuration_publication_error_response` to `prodex-control-plane`.
It maps `ConfigurationPublicationError` into status/code/message triples by
reusing the existing control-plane authorization planner and the revisioned
configuration publication planner:

- authorization failures remain forbidden;
- tenant mismatches remain bad requests; and
- stale or duplicate revision publication remains a conflict.

The composed planner keeps payload contents, tenant identifiers, policy revision
identifiers, operation/resource topology, credential scope details, and role
names out of client-visible responses.

## Consequences

Control-plane composition roots have a single deterministic response boundary
for configuration publication failures while raw errors remain available for
append-only audit and trusted redacted diagnostics.
