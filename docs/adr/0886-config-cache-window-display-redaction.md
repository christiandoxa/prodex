# ADR 0886: Config cache window display redaction

## Status

Accepted.

## Context

Config cache-window validation rejects invalid refresh, stale, and expiry
ordering. The response planner already returns one stable message, but raw
`Display` output still identified which timing relationship failed.

## Decision

Render all `ConfigCacheWindowError` variants as
`configuration cache window is invalid`, matching the response planner. Keep
typed variants unchanged so callers can still classify invalid ordering in
trusted diagnostics.

Regression coverage pins the exact display string and rejects refresh, stale,
and expiry wording from raw display output.

## Consequences

Control-plane configuration boundaries can safely fall back to raw display
strings without exposing cache timing topology. Diagnostics that need the exact
ordering failure should match typed variants.
