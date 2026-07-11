# 0616. Break-Glass Requires Break-Glass Principal Kind

## Status

Accepted

## Context

Break-glass access is intentionally separate from ordinary data-plane and
control-plane credentials. The authz boundary already required the
`BreakGlass` credential scope for `BreakGlassAdmin`, but a service account or
user principal with that scope could still satisfy the boundary.

That weakens the separation requirement because scope alone is easier to
misissue than a canonical break-glass identity type.

## Decision

`prodex-authz` now requires `PrincipalKind::BreakGlass` in addition to
`CredentialScope::BreakGlass` and `Role::Admin` for the `BreakGlassAdmin`
boundary.

The denial uses a stable, redacted `principal_kind_not_allowed` response plan.

## Consequences

- Ordinary users and service accounts cannot become break-glass admins by
  receiving only the break-glass scope.
- Break-glass issuance must create a canonical break-glass principal.
- Existing break-glass compatibility shims must map to `PrincipalKind::BreakGlass`
  before calling the enterprise authz boundary.
