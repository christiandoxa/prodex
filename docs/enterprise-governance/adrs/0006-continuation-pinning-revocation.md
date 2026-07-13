# ADR 0006: Continuation Pinning and Revocation

- Status: Accepted
- Scope: response chains, turn state and session-scoped routes

## Context

Continuation consistency requires affinity, while compliance and incident
response require explicit revocation to take precedence. Mid-stream movement
would violate transport transparency.

## Decision

Bind `previous_response_id`, `x-codex-turn-state` and session-scoped continuation
keys to tenant plus owning provider/profile using bounded opaque records with
expiry and revision epochs. Valid hard affinity bypasses fresh load/health/cap
heuristics. Before commitment, a revoked or no-longer-eligible owner causes a
stable deny/restart-required result or policy-defined eligible recovery; it
never silently selects an ineligible provider. After commitment, never retry or
rotate: expose the natural transport failure. Revocation, tenant mismatch,
expiry and invalidated policy/registry epoch override caches.

## Consequences

Bindings must be merge-safe across processes, tenant-scoped and free of raw
content. Tests cover affinity under backoff/saturation, explicit revocation,
expiry, restart and concurrent persistence.

## Implementation status

Runtime response/session affinity, tenant-bound governance session state,
provider revocation precedence and no-midstream-rotate behavior are wired into
the candidate. Evidence includes
`governed_routing_keeps_eligible_continuation_affinity_ahead_of_soft_score`,
`explicit_provider_revocation_overrides_continuation_affinity`,
`session_reuse_with_another_principal_is_revoked`, and
`explicit_quota_codes_rotate_only_before_commit`. Durable multi-replica session
and revocation validation remains pending with PostgreSQL.
