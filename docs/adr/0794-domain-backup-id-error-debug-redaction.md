# ADR 0794: Redact backup id error debug output

Status: Accepted

## Context

`BackupIdError::InvalidCharacter` carries the rejected character index and
value. Its derived `Debug` formatter exposed those validation details to
diagnostics and containing debug output.

## Decision

Use a custom `Debug` implementation for `BackupIdError` that preserves failure
shape while redacting rejected indexes and characters.

## Consequences

Diagnostics can still distinguish empty backup IDs from invalid-character
failures, but raw validation details no longer appear through `BackupIdError`
debug output.
