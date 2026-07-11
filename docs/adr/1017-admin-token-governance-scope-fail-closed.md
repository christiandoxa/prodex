# ADR 1017: Admin token governance scopes fail closed

## Status

Accepted.

## Context

Configured gateway admin tokens can be scoped by tenant, team, project, user,
and budget. Invalid scope strings were silently discarded during launch
configuration. Dropping a malformed `tenant_id` or other governance dimension
could turn an intended scoped admin token into a broader, partially scoped, or
unscoped control-plane credential.

## Decision

Admin token governance scope fields now fail closed. If `tenant_id`, `team_id`,
`project_id`, `user_id`, or `budget_id` is present, it must be a non-empty
string without whitespace. Invalid values reject gateway launch configuration
instead of being dropped.

## Consequences

Malformed admin-token governance scopes can no longer widen control-plane
access. Deployments must remove absent scopes or provide exact scope values.
Existing valid scope values continue to work unchanged. Regression coverage
lives in `crates/prodex-app/tests/src/app_commands/runtime_launch.rs`.
