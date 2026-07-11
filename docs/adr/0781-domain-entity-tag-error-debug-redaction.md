# ADR 0781: Redact entity tag error debug output

## Status

Accepted

## Context

`EntityTagError` carries rejected token lengths, byte indexes, and invalid
characters for optimistic-concurrency precondition validation. Derived `Debug`
exposed those request-controlled details in diagnostics.

## Decision

Implement custom `Debug` for `EntityTagError`. Preserve the validation failure
variant shape while redacting lengths, indexes, and rejected characters.

## Consequences

Diagnostics still distinguish empty, overlong, and invalid-character entity tag
failures. Raw token length and character details no longer appear through
entity-tag error debug formatting.
