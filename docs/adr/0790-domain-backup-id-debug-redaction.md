# ADR 0790: Redact backup id debug output

Status: Accepted

## Context

`BackupId` stores operator-facing recovery metadata. Its derived `Debug`
formatter exposed raw backup identifiers to diagnostics and containing debug
output.

## Decision

Use a custom `Debug` implementation for `BackupId` that preserves identifier
presence while redacting the raw value.

## Consequences

Diagnostics can still identify backup-id-bearing values, but raw recovery
identifiers no longer appear through `BackupId` debug output.
