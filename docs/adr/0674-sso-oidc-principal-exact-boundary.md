# ADR 0674: SSO and OIDC Principals Match Exactly

## Status

Accepted.

## Context

Gateway SSO/OIDC admin authentication uses upstream principal strings to locate
SCIM-provisioned users. SCIM `userName` is already exact and whitespace-free,
but SSO user headers, OIDC string claims, and SCIM lookup normalized leading or
trailing whitespace before matching.

Normalizing request-controlled or IdP-controlled principal strings can map a
padded principal onto a canonical SCIM user and inherit that user's role and
scope.

## Decision

SSO user headers, OIDC string claims used by gateway admin authentication, and
SCIM user lookup names must be exact non-empty strings without whitespace.
Missing SSO user headers still use the legacy `sso-admin` default.

## Consequences

Padded SSO/OIDC principals no longer match canonical SCIM users. Canonical
principal values remain unchanged, and missing SSO user headers keep their
legacy fallback identity.
