# ADR 0826: Redact domain policy activation state debug output

Status: Accepted

## Context

`PolicyActivationState` stores the active and last-known-good validated policy
snapshots. Derived `Debug` output exposed full state internals whenever cache
activation diagnostics were rendered.

## Decision

Use a custom `Debug` implementation for `PolicyActivationState` that reports
only whether active and last-known-good snapshots are present.

## Consequences

Diagnostics can still distinguish empty, partially populated, and populated
policy activation state without exposing policy payloads, revision identifiers,
or integrity metadata through activation-state debug output.
