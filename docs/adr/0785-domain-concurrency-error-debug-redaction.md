# ADR 0785: Redact concurrency error debug output

## Status

Accepted

## Context

`ConcurrencyError` carries expected and actual resource versions or entity tags
for mutation precondition checks. Derived `Debug` exposed those control-plane
precondition details in diagnostics.

## Decision

Implement custom `Debug` for `ConcurrencyError`. Preserve the precondition
failure variant shape while redacting expected and actual version or entity-tag
values.

## Consequences

Diagnostics still distinguish missing preconditions, version mismatches, and
entity-tag mismatches. Resource versions and entity tags no longer appear
through concurrency-error debug formatting.
