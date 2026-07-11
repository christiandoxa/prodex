# ADR 0795: Redact migration version error debug output

Status: Accepted

## Context

`MigrationVersionError::InvalidCharacter` carries the rejected character index
and value. Its derived `Debug` formatter exposed those validation details to
diagnostics and containing debug output.

## Decision

Use a custom `Debug` implementation for `MigrationVersionError` that preserves
failure shape while redacting rejected indexes and characters.

## Consequences

Diagnostics can still distinguish empty migration versions from
invalid-character failures, but raw validation details no longer appear through
`MigrationVersionError` debug output.
