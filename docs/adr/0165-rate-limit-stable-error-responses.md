# 0165: Rate Limit Stable Error Responses

## Status

Accepted

## Context

The enterprise data plane must reject over-limit requests before provider
dispatch and must express distributed rate-limit mutations as atomic adapter
operations. Raw rate-limit decisions and atomic-update failures include tenant
IDs, virtual-key bucket keys, reset timestamps, remaining capacity, and update
internals. Those details are useful for diagnostics but should not be exposed as
client-visible API details.

## Decision

`prodex-domain` owns stable rate-limit response planners:

- `plan_rate_limit_decision_error_response`
- `plan_rate_limit_atomic_update_error_response`

Rejected rate-limit decisions map to `rate_limit_exceeded` with an optional
`retry_after_seconds` value. Invalid atomic update plans map to stable invalid
request responses. The planners deliberately omit tenant IDs, virtual-key IDs,
bucket cache keys, reset timestamps, remaining capacity, and usage counters.

## Consequences

- Gateway admission can reject rate-limited requests before upstream dispatch
  using one stable response boundary.
- Distributed limiter adapters can keep atomic mutation details internal.
- Client-visible responses remain machine-readable without exposing tenant or
  bucket topology.
