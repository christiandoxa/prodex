# ADR 0836: Redact domain migration compatibility debug output

Status: Accepted

## Context

`MigrationCompatibilityWindow` carries target and compatible source migration
versions. Derived `Debug` output exposed the exact version strings through
diagnostics.

## Decision

Use a custom `Debug` implementation for `MigrationCompatibilityWindow` that
preserves target shape and compatible-source count while relying on redacted
migration-version output.

## Consequences

Diagnostics can still distinguish compatibility-window shape, but exact target
and source version strings no longer appear through compatibility-window debug
output.
