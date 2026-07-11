# ADR 1012: SSO SCIM tenant-claim isolation

## Status

Accepted.

## Context

Gateway SSO and OIDC admin authentication can enrich an authenticated user with
SCIM role, scope, and key-prefix metadata. The lookup was username-only and ran
before applying an explicit tenant claim. A token carrying `tenant-b` could
therefore reuse SCIM metadata stored for the same username under `tenant-a`.
That could leak tenant-scoped admin role or key-prefix privileges across tenant
boundaries.

## Decision

When an SSO header or OIDC claim supplies a tenant ID, SCIM fallback metadata is
used only if the matched SCIM user has the same tenant ID. Mismatched SCIM
records are ignored, so missing/unknown role claims continue to resolve to
Viewer instead of inheriting Admin from another tenant. If no tenant claim is
present, the existing SCIM tenant fallback remains for compatibility.

## Consequences

Explicit tenant claims now bound SCIM role enrichment to the same tenant. Legacy
deployments that depend on SCIM to supply the tenant while omitting a tenant
claim keep working. This does not replace the longer-term requirement for
canonical typed `TenantContext` and `PrincipalId` through admin auth. Regression
coverage lives in
`crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gateway_admin_auth.rs`.
