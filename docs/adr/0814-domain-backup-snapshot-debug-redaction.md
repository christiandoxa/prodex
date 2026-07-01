# ADR 0814: Redact backup snapshot debug output

Status: Accepted

## Context

`BackupSnapshot` carries recovery identifiers, retention timestamps, status, and
optional checksum evidence. Its derived `Debug` formatter exposed raw backup
metadata and checksum values through diagnostics.

## Decision

Use a custom `Debug` implementation for `BackupSnapshot` that preserves backup
status and identifier presence while redacting raw identifiers, timestamps, and
checksums.

## Consequences

Diagnostics can still distinguish backup status, but raw recovery evidence no
longer appears through `BackupSnapshot` debug output.
