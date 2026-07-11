# 0636: Restore Expiry Window Guard

## Status

Accepted

## Context

Restore validation rejected expired backups based on `now >= expires_at`, but a
backup snapshot with `expires_at <= created_at` could still validate when
`now` was earlier than the invalid expiry value.

## Decision

`RestorePlan::validate` now rejects backup snapshots whose expiry timestamp is
not after the creation timestamp. The response planner maps this to
`restore_backup_expiry_invalid`.

## Consequences

- Invalid backup retention windows cannot be treated as restorable snapshots.
- Backup timestamps stay out of client-visible restore errors.
- Backup/restore runbooks can rely on a strictly positive restore window.
