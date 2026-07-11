# 0627: Policy Stale Fallback Guard

## Status

Accepted

## Context

Gateway policy caches need explicit revision, refresh, invalidation, and
last-known-good behavior. The domain refresh evaluator selected
`UseLastKnownGoodAndRefresh` for any stale cache window, even when no
last-known-good revision existed or that fallback revision had been explicitly
invalidated.

## Decision

`evaluate_policy_refresh` now selects last-known-good only when
`last_known_good_revision_id` is present and does not match the invalidated
revision marker. Stale caches without a usable fallback return `RefreshAsync`
until expiry.

## Consequences

- Adapters cannot claim last-known-good behavior when no safe fallback revision
  exists.
- Explicit invalidation cannot resurrect a withdrawn fallback revision.
- Expired caches still fail closed through the existing policy error planner.
