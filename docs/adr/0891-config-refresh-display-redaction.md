# ADR 0891: Config refresh display redaction

## Status

Accepted.

## Context

Configuration refresh response planning already returns one stable message for
refresh-required and invalidated-revision cache states. Raw `Display` output
still distinguished required refresh from unavailable invalidated revision.

## Decision

Render every `ConfigRefreshError` variant as
`configuration is not currently available`, matching the response planner. Keep
typed variants and response codes unchanged for trusted diagnostics.

Regression coverage pins the exact display string and rejects refresh,
invalidated, and revision wording from raw display output.

## Consequences

Config readiness boundaries can safely fall back to raw display strings without
exposing cache-state classification. Diagnostics should continue matching typed
variants when exact cache state matters.
