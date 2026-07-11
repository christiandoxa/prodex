# ADR 0916: Config publication error debug redaction

## Status

Accepted.

## Context

Configuration publication errors carry expected/actual tenant IDs and current
or candidate revision IDs. Display and response planners already use stable
messages, but derived `Debug` output exposed those identifiers.

## Decision

Use a custom `Debug` implementation for `ConfigPublicationError` that preserves
variant shape while redacting tenant and revision identifiers. Keep typed
fields available for response planning and tests.

Regression coverage rejects raw tenant IDs and revision IDs in rendered
publication-error debug output.

## Consequences

Configuration publication diagnostics can distinguish failure class without
exposing tenant topology or revision metadata. Error handling behavior remains
unchanged.
