# 0654: OIDC Issuer and Audience Exact Boundary

## Status

Accepted

## Context

`Issuer::new` and `Audience::new` trimmed input before validation. OIDC issuer
and audience values are security-sensitive identity metadata, so accepting
padded values can hide malformed configuration and weaken exact claim matching.

## Decision

`Issuer::new` and `Audience::new` now validate the exact provided value.
Only zero-length values are empty errors; whitespace-only, leading-whitespace,
and trailing-whitespace values are rejected as invalid input. Issuer trailing
slash canonicalization is retained for the existing issuer URL contract.

## Consequences

- OIDC issuer and audience configuration is exact at the domain boundary.
- Existing valid issuer and audience values keep working.
- Stable identity error responses continue to redact rejected raw values.
