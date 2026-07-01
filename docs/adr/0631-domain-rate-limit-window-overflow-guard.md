# 0631: Domain Rate-Limit Window Overflow Guard

## Status

Accepted

## Context

Rate-limit evaluation converts `window_seconds` into milliseconds to compute a
reset horizon. The previous implementation used saturating arithmetic, so an
oversized window could silently turn into an unrealistic reset timestamp.

## Decision

`evaluate_rate_limit` now rejects rules with `window_seconds > u64::MAX / 1000`.

## Consequences

- Invalid rate-limit windows fail closed before admission.
- Distributed limiter adapters do not receive allowances derived from saturated
  reset math.
- Existing zero-capacity and zero-window behavior remains unchanged.
