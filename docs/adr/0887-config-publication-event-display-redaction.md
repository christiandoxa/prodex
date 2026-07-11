# ADR 0887: Config publication event display redaction

## Status

Accepted.

## Context

Configuration publication events must include gateway cache refresh and runtime
policy reload targets. The response planner already exposes one stable message
for incomplete events, but raw `Display` output identified which target was
missing.

## Decision

Render all `ConfigPublicationEventError` variants as
`configuration publication event is incomplete`, matching the response planner.
Keep typed variants unchanged so trusted delivery code can still classify the
missing target.

Regression coverage pins the exact display string and rejects gateway, cache,
runtime, and policy wording from raw display output.

## Consequences

Control-plane publication boundaries can safely fall back to raw display strings
without exposing local cache or runtime-policy topology. Diagnostics should
match typed variants for target-level detail.
