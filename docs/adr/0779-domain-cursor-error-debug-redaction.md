# ADR 0779: Redact cursor error debug output

## Status

Accepted

## Context

`CursorError` carries rejected cursor lengths, byte indexes, and invalid
characters for pagination validation. Derived `Debug` exposed those request
details in diagnostics.

## Decision

Implement custom `Debug` for `CursorError`. Preserve the cursor validation
failure variant shape while redacting lengths, indexes, and rejected
characters.

## Consequences

Diagnostics still distinguish empty, overlong, and invalid-character cursor
failures. Raw cursor length and character details no longer appear through
cursor-error debug formatting.
