# 0623. MigrationVersion Character Guard

## Status

Accepted

## Context

Migration versions are used by external migrators, startup compatibility checks,
status output, and operator runbooks. The domain boundary rejected empty
versions, but accepted whitespace, control characters, and non-ASCII text.

Those values make version comparison and shell/database reporting ambiguous.

## Decision

`MigrationVersion::new` now accepts only ASCII graphic characters after
trimming. Invalid characters produce `MigrationVersionError::InvalidCharacter`.

The existing `plan_migration_version_error_response` maps this to the same
redacted `migration_version_invalid` response.

## Consequences

- Migration versions remain stable across CLI, database metadata, logs, and
  Kubernetes migration jobs.
- Raw invalid characters and offsets stay out of public errors.
- Storage adapters can still impose stricter backend-specific migration naming
  rules if needed.
