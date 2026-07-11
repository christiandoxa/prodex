# 0653: Backup and Migration Identifier Exact Boundary

## Status

Accepted

## Context

`BackupId::new` and `MigrationVersion::new` trimmed input before validation and
storage. Backup IDs and migration versions are operational identifiers used in
restore drills, migration status, compatibility checks, and diagnostics, so
silently normalizing padded values can hide malformed callers.

## Decision

`BackupId::new` and `MigrationVersion::new` now validate and store the exact
provided value. Empty strings remain empty errors. Whitespace-only, leading, or
trailing whitespace values are rejected as invalid characters.

## Consequences

- Backup and migration identifiers stay exact at the domain boundary.
- Existing valid identifiers keep working.
- Stable error responses continue to redact rejected raw values.
