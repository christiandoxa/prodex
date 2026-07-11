# 0619. Restore Checksum Blank Guard

## Status

Accepted

## Context

The restore boundary already rejected missing backup checksums and mismatched
expected checksums. A blank checksum value could still be present and match a
blank expected value.

That weakens backup/restore drill evidence because "present but empty" is not an
integrity proof.

## Decision

`RestorePlan::validate` now rejects blank actual or expected checksum values with
`RestorePlanError::MalformedChecksum`.

The stable response planner exposes `restore_checksum_invalid` without returning
the checksum value.

## Consequences

- Restore plans cannot treat empty checksum material as verified integrity.
- Public restore validation responses remain redacted.
- A future checksum-format parser can replace this guard when storage adapters
  standardize digest algorithms.
