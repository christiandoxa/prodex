# ADR 0482: Missing SSO roles never default to admin

## Status

Accepted.

## Context

Gateway SSO/OIDC admin auth used `gateway.sso.default_role` after a missing or
unmapped role claim. If that default was configured as `admin`, a missing or
unknown external claim could become an admin principal.

## Decision

Resolve admin role only from an explicit SSO/OIDC role claim or an explicit SCIM
user role. Missing or unknown roles resolve to `viewer`.

## Consequences

Missing or malformed role claims can still authenticate for read-only admin
views, but cannot perform write operations. Existing explicit admin claims and
SCIM admin roles keep working.
