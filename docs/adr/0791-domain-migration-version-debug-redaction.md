# ADR 0791: Redact migration version debug output

Status: Accepted

## Context

`MigrationVersion` stores operational migration identifiers. Its derived
`Debug` formatter exposed raw migration names and sequencing details to
diagnostics and containing debug output.

## Decision

Use a custom `Debug` implementation for `MigrationVersion` that preserves
identifier presence while redacting the raw value.

## Consequences

Diagnostics can still identify migration-version-bearing values, but raw
migration identifiers no longer appear through `MigrationVersion` debug output.
