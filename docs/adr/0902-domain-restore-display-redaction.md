# ADR 0902: Domain restore display redaction

## Status

Accepted.

## Context

Restore planning validates backup status, expiry windows, and checksum
material before recovery workflows proceed. The response planner already
returns stable restore messages, but raw `Display` output still distinguished
not-completed, expired, missing-checksum, malformed-checksum, and mismatch
classes.

## Decision

Render `RestorePlanError` with the same messages used by
`plan_restore_error_response`. Keep typed variants and response codes unchanged
for trusted classification.

Regression coverage pins exact display strings for the backup-restorability,
checksum-verification, and checksum-mismatch cases.

## Consequences

Backup/restore boundaries can safely fall back to raw display strings without
exposing restore validation shape or checksum availability details. Diagnostics
should continue matching typed variants for exact reasons.
