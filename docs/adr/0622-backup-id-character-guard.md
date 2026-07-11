# 0622. BackupId Character Guard

## Status

Accepted

## Context

Backup IDs are operator-facing recovery metadata that can also appear in API
requests, audit trails, object-store keys, and runbook evidence. The domain
boundary already rejected empty backup IDs, but accepted whitespace, control
characters, and non-ASCII text.

Those values make backup/restore evidence harder to compare across logs,
storage backends, and shell runbooks.

## Decision

`BackupId::new` now accepts only ASCII graphic characters after trimming.
Invalid characters produce `BackupIdError::InvalidCharacter`.

The existing `plan_backup_id_error_response` maps this to the same redacted
`backup_id_invalid` response.

## Consequences

- Backup and restore identifiers remain transport- and log-stable.
- Raw invalid characters and offsets stay out of public API errors.
- Storage adapters still own backend-specific object-key/path mapping.
