# 0659. Authn Exact JWKS URL and Key ID Boundary

## Status

Accepted

## Context

OIDC authentication is a trust boundary. The authn crate already avoids network
work on the gateway request path and validates configured JWKS refresh targets,
but configured JWKS URLs were trimmed before validation and known token key IDs
were accepted when they contained printable or non-printable padding.

Normalizing these values can hide malformed configuration or token metadata.

## Decision

`prodex-authn` validates configured JWKS URLs exactly as supplied. Padded URLs
are invalid instead of normalized. Refresh URLs must be non-empty printable
ASCII values with a 2048-byte maximum.

`authenticate_oidc_claims` treats token key IDs as unknown unless they are
non-empty printable ASCII values with a 128-byte maximum.

## Consequences

- Background JWKS refresh planning keeps rejecting non-HTTPS, cross-issuer,
  whitespace-bearing, non-printable, non-ASCII, overlong, and padded configured
  URLs before any transport adapter can fetch them.
- Malformed token key IDs fail closed as `UnknownKeyId` with the existing
  redacted stable authentication response.
- Existing valid printable ASCII key IDs continue to authenticate.
