# ADR 0788: Redact entity tag debug output

Status: Accepted

## Context

`EntityTag` stores an optimistic-concurrency token provided by clients or derived
from a resource version. Its derived `Debug` formatter exposed the raw ETag value
to diagnostics and any containing debug output.

## Decision

Use a custom `Debug` implementation for `EntityTag` that preserves the presence
of an entity tag while redacting the token value.

## Consequences

Diagnostics can still distinguish entity-tag-bearing values from missing values,
but raw precondition tokens no longer appear through `EntityTag` debug output.
