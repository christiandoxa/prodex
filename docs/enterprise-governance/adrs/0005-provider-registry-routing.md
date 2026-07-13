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

The local rewrite runtime publishes a revisioned tenant-bound snapshot for the
single provider adapter attached to that process. It advertises only endpoints
and capabilities implemented by that adapter, hard-filters disabled or revoked
entries, preserves eligible continuation affinity, and requires a successful
audited governed decision before enforcing-mode dispatch. Existing adapter-local
health, quota, circuit and bounded precommit retry machinery remains authoritative.

The current process architecture owns one `RuntimeLocalRewriteProviderOptions`
adapter, one upstream configuration, and one credential family. Simultaneous
heterogeneous provider dispatch is therefore not executable in one process.
Configured or stale routes naming another provider must fail unavailable; they
must never be sent through the attached adapter. Multi-provider fallback requires
a broker/process boundary that owns one executable adapter per candidate. Until
that exists, precommit retries remain within the original one-provider eligible
snapshot and no cross-provider fallback is advertised.

Evidence includes `governed_routing_enforces_every_hard_eligibility_gate`,
`governed_routing_scores_in_fixed_point_and_breaks_ties_by_provider`,
`governed_routing_does_not_preserve_affinity_after_revocation`, and the
provider-SPI/production boundary guards.
