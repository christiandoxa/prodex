# ADR 1018: Virtual key governance scopes fail closed

## Status

Accepted.

## Context

Configured gateway virtual keys can carry tenant, team, project, user, and
budget scope metadata. Invalid scope strings were silently discarded during
launch configuration. Dropping a malformed `tenant_id` or other governance
dimension could turn an intended tenant-owned key into a broader data-plane
credential.

## Decision

Virtual key governance scope fields now fail closed. If `tenant_id`, `team_id`,
`project_id`, `user_id`, or `budget_id` is present, it must be a non-empty
string without whitespace. Invalid values reject gateway launch configuration
instead of being dropped.

## Consequences

Malformed virtual-key governance scopes can no longer widen data-plane access
or accounting scope. Deployments must remove absent scopes or provide exact
scope values. Existing valid scope values continue to work unchanged.
Regression coverage lives in
`crates/prodex-app/tests/src/app_commands/runtime_launch.rs`.
