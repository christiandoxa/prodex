# ADR 0665: Gateway Admin Scope Exact Boundary

## Status

Accepted.

## Context

Legacy gateway SCIM provisioning, SSO/OIDC admin authentication, and gateway key
CRUD accept optional tenant, team, project, user, and budget scope IDs plus
allowed key prefixes. These fields feed admin authorization and key creation
scope. Trimming them at the boundary can turn a padded request-controlled value
such as ` tenant-a ` into `tenant-a`, which weakens exact tenant and
governance-scope matching.

## Decision

SCIM, SSO header, OIDC claim, and gateway key CRUD scope IDs and allowed key
prefixes must be exact non-empty strings without whitespace when present. `null`
remains the compatibility signal for clearing an optional scope where the API
supports clearing. Display-oriented optional strings such as `displayName` and
`externalId` keep their existing normalization behavior.

## Consequences

Padded or whitespace-bearing SCIM scope IDs and key prefixes fail with
`invalid_scim_field`; padded gateway key CRUD scope fields fail the same stable
validation boundary; padded or empty SSO/OIDC admin-auth scope values fail
closed instead of being normalized into an authorized or unscoped admin scope.
Legacy clients that need to clear a supported optional scope must send `null`.
