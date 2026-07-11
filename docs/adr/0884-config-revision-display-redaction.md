# ADR 0884: Config revision display redaction

## Status

Accepted.

## Context

Configuration publication and invalidation response planners already expose
stable messages for stale or unknown revisions. Raw `Display` output still used
local implementation wording such as cache membership and active-revision state.

## Decision

Render config revision publication and invalidation failures with the same
stable client-facing wording used by their response planners. Keep typed
variants and response codes unchanged so callers can still classify stale
publication and unknown invalidation attempts.

Regression coverage pins the exact display strings and rejects revision IDs plus
internal `config revision` and cache wording.

## Consequences

Control-plane boundaries can safely fall back to raw display output without
exposing cache topology or active-revision implementation detail. Trusted
diagnostics should keep matching typed variants.
