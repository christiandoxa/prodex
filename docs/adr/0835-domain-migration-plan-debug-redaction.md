# ADR 0835: Redact domain migration plan debug output

Status: Accepted

## Context

`MigrationStep` and `MigrationPlan` carry migration versions, descriptions, and
lock owners. Derived `Debug` output exposed those operational details through
diagnostics.

## Decision

Use custom `Debug` implementations for migration steps and plans that preserve
execution mode, step state, step kind, and step count while redacting
descriptions and lock owners and relying on redacted migration-version output.

## Consequences

Diagnostics can still distinguish migration shape and ordering state, but raw
migration descriptions, lock owners, and version strings no longer appear
through migration plan debug output.
