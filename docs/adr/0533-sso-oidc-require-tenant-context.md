# ADR 0533: Require tenant context for multi-tenant SSO and OIDC

## Status

Accepted

## Context

Multi-tenant gateway deployments need every authenticated admin principal to
resolve to a tenant before authorization. Existing trusted-proxy SSO and OIDC
paths could still authenticate an unscoped principal when no tenant header,
tenant claim, or active SCIM user tenant existed.

## Decision

Add `gateway.sso.require_tenant`. When enabled, trusted-proxy SSO and native
OIDC admin authentication fail closed unless tenant context is supplied by the
configured trusted header, configured OIDC claim, or active SCIM user record.

The default remains `false` for local and existing single-tenant compatibility.
Production multi-tenant deployments should set it to `true`.

## Consequences

Multi-tenant deployments can require tenant-scoped admin principals without
breaking existing local/single-tenant gateway configurations.
