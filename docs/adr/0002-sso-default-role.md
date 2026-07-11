# ADR 0002: SSO default role is Viewer

## Status

Accepted.

## Context

Phase 0 security audit requires missing or unknown role claims to deny access or
fall back to the least-privileged role, never to `Admin`.

Prodex gateway SSO can derive an admin principal from trusted proxy headers,
SCIM user metadata, or OIDC claims. When `gateway.sso.default_role` was absent,
runtime gateway configuration previously defaulted SSO-authenticated requests to
`Admin`, making a missing upstream role claim a vertical privilege-escalation
risk.

## Decision

The runtime gateway SSO default role is now `Viewer`.

Admin access must be explicit: a trusted SSO header, SCIM user role, or OIDC role
claim must resolve to `admin`. The domain role mapper also keeps missing or
unknown role claims from mapping to `Admin`.

## Consequences

- Existing deployments that intentionally relied on implicit SSO admin access
  must provide an explicit admin role claim or SCIM admin role.
- Missing role claims remain authenticated but read-only by default.
- Mutating control-plane routes continue to require an admin role.
