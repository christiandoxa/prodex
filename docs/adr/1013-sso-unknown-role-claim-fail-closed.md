# ADR 1013: SSO unknown role claim fail-closed

## Status

Accepted.

## Context

Gateway SSO and OIDC admin authentication resolve a role from the incoming role
claim/header and can fall back to SCIM user metadata. Missing role claims may
use SCIM as an explicit mapping, but an unknown or malformed role claim was also
falling through to SCIM. A token with a bad role value could therefore inherit
SCIM Admin privileges instead of failing closed.

## Decision

An explicit SSO/OIDC role claim now wins. If the claim is unknown, malformed,
non-string, or whitespace-padded, the resolved role is Viewer and SCIM role
metadata is not consulted. The OIDC parser distinguishes an absent role claim
from a present invalid role claim so only absence can use SCIM role fallback.
SCIM role fallback remains available only when the role claim/header is absent.

## Consequences

Malformed or unknown SSO/OIDC role claims can no longer become Admin through
SCIM fallback. Deployments that intentionally rely on SCIM role mapping should
omit the role claim/header rather than sending an invalid value. Regression
coverage lives in
`crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gateway_admin_auth.rs`.
