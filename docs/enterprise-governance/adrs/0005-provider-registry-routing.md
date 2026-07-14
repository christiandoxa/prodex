# ADR 0005: Provider Registry and Governed Routing

- Status: Accepted
- Scope: target provider selection

## Context

Existing profile/provider selection, affinity, retry and circuit primitives do
not constitute a revisioned compliance registry or an auditable governed
routing decision.

## Decision

Publish immutable approved provider-registry snapshots containing provider,
endpoint/model capability, region/residency, retention/training posture,
classification ceiling, trust tier, credential reference, health source and
revision metadata. Routing first applies hard policy/obligation filters. It then
scores only the eligible set using bounded deterministic integer/fixed-point
inputs such as affinity, health, capacity, latency and cost. Stable tie-breaks
make the choice reproducible. Precommit retry/fallback remains inside the same
eligible set. No rotation occurs after response commit.

## Consequences

A soft score can never restore an ineligible provider. Decisions record
content-free hard-filter reasons, score components, selected target and snapshot
revision. Revocation overrides continuation pinning.

## Implementation status

The local rewrite runtime publishes a revisioned tenant-bound snapshot for
eligible executable adapters. It advertises only implemented endpoints and
capabilities, hard-filters disabled or revoked entries, projects bounded runtime
signals, preserves eligible continuation affinity, and requires a successful
audited decision before enforcing-mode dispatch. Heterogeneous projected
credentials resolve to their matching adapter, and fallback remains inside the
original eligible snapshot before response commitment. Unbound or stale routes
fail unavailable and are never reinterpreted through another adapter.

Evidence includes `governed_routing_enforces_every_hard_eligibility_gate`,
`governed_routing_scores_in_fixed_point_and_breaks_ties_by_provider`,
`governed_routing_does_not_preserve_affinity_after_revocation`, and the
`provider_registry_resolves_selected_heterogeneous_projected_adapter` regression
plus provider-SPI/production boundary guards.
