# ADR 0455: Config refresh display errors are redacted

## Status

Accepted.

## Context

Configuration refresh response planning already returns a stable generic error
for unavailable or invalidated revisions. Local `Display` text still named the
invalidated-revision state, which can leak policy-cache internals if surfaced by
a composition root.

## Decision

`ConfigRefreshError::InvalidatedRevisionRejected` now uses a generic display
message. Structured variants remain available for trusted diagnostics and tests.

## Consequences

Accidental display paths no longer expose whether configuration was rejected
because of an invalidated revision.
