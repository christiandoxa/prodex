# ADR 0280: Runtime Policy Redacted Cache Invalidation Plan

## Status

Accepted.

## Context

Runtime policy cache reload is triggered by configuration publication events.
The cache already supports per-root invalidation and explicit reload, but the
primitive invalidation API returns the entire cached policy value. That is useful
for compatibility and existing tests, but event consumers and logs only need to
know whether a root cache entry existed and which policy version was evicted.

Returning full policy material from event-handling paths increases the chance
that gateway, secret, or provider configuration is accidentally logged while
diagnosing cache refresh behavior.

## Decision

Add `plan_runtime_policy_cache_invalidation(root)` to
`prodex-runtime-policy`. The function invalidates the selected root cache entry
and returns a `RuntimePolicyCacheInvalidationPlan` containing only:

- the affected root path;
- whether a cached entry existed; and
- the evicted policy file version, when the entry contained a loaded policy.

The existing `invalidate_runtime_policy_cache_for(root)` API remains available
for compatibility, while new publication-event adapters should prefer the
redacted plan.

## Consequences

- Cache refresh handlers can produce operational evidence without exposing full
  runtime policy contents.
- Explicit reload semantics remain unchanged.
- Multi-replica propagation still belongs to the production event transport.
