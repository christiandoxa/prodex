# ADR 0068: Domain policy cache refresh and invalidation

## Status

Accepted.

## Context

The enterprise target requires policy/config cache behavior with immutable
revision IDs, refresh timing, invalidation, and last-known-good semantics. The
domain crate already required signed/validated policy snapshots and activation
state, but it did not model when a gateway should keep the active policy, refresh
asynchronously, fall back to last-known-good, expire, or stop using an
invalidated revision.

## Decision

Add pure domain policy cache status and refresh decision types:
`PolicyRefreshWindow`, `PolicyCacheStatus`, `PolicyRefreshDecision`, and
`evaluate_policy_refresh`. The refresh window enforces ordered refresh, stale,
and expiry timestamps. Invalidated active revisions take precedence over normal
refresh timing, followed by expiry, stale-with-last-known-good, asynchronous
refresh, then normal active use.

## Consequences

- Gateway and future control-plane code can share one revision-aware policy cache
  decision model without coupling `prodex-domain` to timers, HTTP, databases, or
  filesystems.
- Readiness/diagnostic endpoints can expose active and last-known-good revision
  IDs from the same status object.
- Future signed policy distribution work can add async refresh transport while
  keeping this side-effect-free decision logic stable.
