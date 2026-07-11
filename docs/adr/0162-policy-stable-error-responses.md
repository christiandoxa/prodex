# 0162: Policy Stable Error Responses

## Status

Accepted

## Context

The enterprise gateway uses signed, revisioned policy snapshots with
last-known-good fallback. Policy activation, refresh-window validation, and
cache decisions can fail for reasons that include digest verification,
signature verification, invalid timing windows, expiry, and deliberate
invalidation.

Those details are useful for audit logs and operators, but they must not become
client-visible API details. Exposing revision IDs, digest or signature values,
cache timing, invalidation targets, or last-known-good internals would make
policy topology observable and could leak security-sensitive metadata.

## Decision

`prodex-domain` owns stable response planners for policy boundary failures:

- `plan_policy_activation_error_response`
- `plan_policy_refresh_window_error_response`
- `plan_policy_refresh_decision_error_response`

The planners return small, serializable response plans with stable status,
code, and message fields. They deliberately do not include revision IDs,
digest/signature material, refresh/stale/expiry timestamps, invalidated
revision IDs, or last-known-good cache state.

Adapters may map the status enum to protocol-specific HTTP or RPC status codes,
but they must use these planners before exposing policy activation,
configuration validation, readiness, or cache-unavailable failures to clients.

## Consequences

- Policy internals remain available to audit/diagnostic logs without becoming
  part of the public API contract.
- Gateway and control-plane adapters share one redacted policy error boundary.
- Expired or invalidated policy cache states fail closed with stable
  service-unavailable responses instead of leaking cache topology.
