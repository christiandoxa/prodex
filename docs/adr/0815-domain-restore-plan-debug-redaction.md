# ADR 0815: Redact restore plan debug output

Status: Accepted

## Context

`RestorePlan` carries backup snapshot metadata, expected checksum evidence, and
the restore evaluation timestamp. Its derived `Debug` formatter exposed those
raw restore details through diagnostics.

## Decision

Use a custom `Debug` implementation for `RestorePlan` that relies on redacted
backup snapshot output and redacts expected checksums and evaluation time.

## Consequences

Restore validation behavior remains unchanged, but raw restore evidence no
longer appears through `RestorePlan` debug output.
