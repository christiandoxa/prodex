# ADR 0316: Config Refresh Error Response Boundary

## Status

Accepted.

## Context

`prodex-config` already evaluates cache states as active, async-refreshable,
last-known-good, refresh-required, or rejected because the active revision was
invalidated. Adapter code still needs a stable way to turn only the unservable
states into client-visible failures. Exposing cache timing, revision IDs, or
last-known-good state in those responses would leak operational internals.

## Decision

Add `ConfigRefreshError`, `config_refresh_error_for_decision`, and
`plan_config_refresh_error_response` to the config boundary crate.

Only `RefreshRequired` and `RejectedInvalidated` produce errors. Active,
async-refreshable, and last-known-good decisions remain usable and return no
error. Both unservable states map to service-unavailable response plans with
stable codes and the same redacted message.

`ci:config-boundary-guard` now checks that the refresh error types, decision
mapping, and redacted response planner stay present in the boundary crate.

## Consequences

Readiness, admin, and gateway adapters can fail closed when no usable
configuration exists without leaking revision IDs, refresh windows,
invalidation targets, or last-known-good internals. Usable stale or
last-known-good states continue to serve traffic instead of being treated as
client-visible errors.
