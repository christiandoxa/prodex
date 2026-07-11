# ADR 0915: Config invalidation error debug redaction

## Status

Accepted.

## Context

Configuration invalidation errors carry expected/actual tenant IDs and unknown
revision IDs. Display and response planners already use stable messages, but
derived `Debug` output exposed those identifiers.

## Decision

Use a custom `Debug` implementation for `ConfigInvalidationError` that preserves
variant shape while redacting tenant and revision identifiers. Keep typed
fields available for response planning and tests.

Regression coverage rejects raw tenant IDs and revision IDs in rendered
invalidation-error debug output.

## Consequences

Configuration invalidation diagnostics can distinguish failure class without
exposing tenant topology or revision metadata. Error handling behavior remains
unchanged.
