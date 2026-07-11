# ADR 0607: Domain OIDC Issuer Host Guard

## Status

Accepted.

## Context

JWKS refresh planning rejects non-HTTPS and hostless targets, but issuer
construction could still accept values such as `https://`,
`http://idp.example.com`, or `idp.example.com`. A later discovery plan would
then derive an invalid or insecure URL from an invalid issuer.

## Decision

OIDC issuer construction now requires `https://` and a non-empty host. Hostless
or non-HTTPS values are rejected as invalid issuer configuration.

## Consequences

Invalid issuer configuration fails before authentication or background refresh
planning. Existing normalized issuer values keep trimming a trailing slash.
