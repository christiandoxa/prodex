# ADR 0006: Continuation Pinning and Revocation

- Status: Proposed
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

Runtime response/session affinity and no-midstream-rotate behavior exist.
Tenant-bound governance epochs and explicit provider-registry revocation
semantics remain planned.
