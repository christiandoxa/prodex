# ADR 0005: Provider Registry and Governed Routing

- Status: Proposed
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

Provider adapters, affinity, circuit, quota and bounded precommit retry
foundations exist. Revisioned compliance registry, hard-filter and auditable
fixed-point choice remain gaps.
