# ADR 0332: Application Configuration Readiness Snapshot

## Status

Accepted.

## Context

Domain health probes can expose active policy revision metadata, and config cache
state already decides whether to use active, refresh asynchronously, use
last-known-good, require refresh, or reject an invalidated revision. Composition
roots still need one application boundary that maps those config decisions into a
canonical readiness snapshot without duplicating refresh semantics.

## Decision

Add `plan_application_configuration_readiness_snapshot` to
`prodex-application`.

The planner evaluates the `ConfigCacheState` with `evaluate_config_refresh`,
selects the active revision for `UseActive` and `RefreshAsync`, selects the
last-known-good revision for `UseLastKnownGood`, and withholds revision metadata
for `RefreshRequired` or `RejectedInvalidated`. It then builds a domain
`HealthSnapshot` with the provided liveness/startup/draining state and health
checks.

## Consequences

- Readiness adapters can expose active or last-known-good revision metadata from
  one application planner.
- Invalidated or expired config without a usable last-known-good revision fails
  readiness.
- Config refresh policy remains owned by `prodex-config`; application code only
  composes it into health state.
