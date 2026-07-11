# ADR 0911: Config cache state debug redaction

## Status

Accepted.

## Context

Configuration cache state carries tenant IDs, active revision IDs,
last-known-good revision IDs, invalidated revision IDs, and cache timing
windows. Derived `Debug` output exposed that topology through refresh,
activation, and invalidation diagnostics.

## Decision

Use a custom `Debug` implementation for `ConfigCacheState` that redacts tenant
and revision identifiers while delegating cache timing to the redacted
`ConfigCacheWindow` formatter. Keep exact values available through typed fields
for cache refresh and activation logic.

Regression coverage rejects raw tenant IDs, revision IDs, and cache timings in
rendered cache-state debug output.

## Consequences

Configuration cache and activation planners can be inspected without exposing
tenant topology or policy revision metadata. Runtime behavior remains
unchanged.
