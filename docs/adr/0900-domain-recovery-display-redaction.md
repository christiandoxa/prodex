# ADR 0900: Domain recovery display redaction

## Status

Accepted.

## Context

Backup identifiers and audit retention purge batches are recovery-plane
boundaries. Their response planners already expose stable messages, but raw
`Display` output still distinguished empty identifiers, invalid characters,
oversized batches, and cross-tenant purge keys.

## Decision

Render `BackupIdError` and `AuditRetentionPurgeBatchError` with the same
messages used by their response planners. Keep typed variants and response
codes unchanged for trusted classification.

Regression coverage pins exact display strings and keeps existing debug
redaction assertions for rejected indexes, characters, counts, and limits.

## Consequences

Backup/restore and audit retention boundaries can safely fall back to raw
display strings without exposing request validation shape or tenant-scope
failure details. Diagnostics should continue matching typed variants for exact
reasons.
