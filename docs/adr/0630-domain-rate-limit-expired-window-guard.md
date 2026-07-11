# 0630: Domain Rate-Limit Expired Window Guard

## Status

Accepted

## Context

Distributed rate-limit adapters use `RateLimitAtomicUpdate` as the shared plan
for atomic increment and expiry. The planner rejected tenant mismatch and zero
increments, but still accepted an allowance reset time that was already at or
before the request time.

## Decision

`RateLimitAtomicUpdate::from_allowed_request` now rejects expired reset windows
with `RateLimitAtomicUpdateError::ExpiredWindow`. The stable response planner
maps that to `rate_limit_window_invalid`.

## Consequences

- Redis/SQL adapters do not receive atomic update plans with invalid expiry.
- Stale allowance data fails closed before distributed limiter mutation.
- Existing client-facing rate-limit errors remain redacted.
