# 0661. Restore Checksum Exact Boundary

## Status

Accepted

## Context

Restore validation already rejected missing, blank, malformed, and mismatched
checksums. The checksum helper still trimmed values before character validation,
so padded actual and expected checksums could both pass if their raw padded
strings matched.

Backup/restore evidence in regulated environments should not normalize
integrity metadata silently.

## Decision

`RestorePlan::validate` now checks checksum values exactly as supplied.
Checksums must be non-empty printable ASCII values with a 512-byte maximum.

## Consequences

- Padded checksum values are malformed instead of normalized.
- Overlong checksum values fail before restore is treated as verified.
- Public restore validation responses remain redacted through the existing
  stable response planner.
