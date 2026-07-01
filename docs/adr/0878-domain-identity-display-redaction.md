# ADR 0878: Domain identity display redaction

## Status

Accepted.

## Context

Identity response planners already return stable messages for configuration and
token-validation failures. The raw `Display` implementations still identified
issuer, audience, and JWT algorithm failure classes. Those classes are useful
for structured authorization decisions, but raw display text is easy to
accidentally surface at an auth boundary.

## Decision

Render all `IdentityConfigError` variants as
`identity configuration is invalid` and all `TokenValidationError` variants as
`token validation failed`. Keep the existing response planners unchanged so
callers still receive precise low-cardinality error codes by matching variants.

Regression coverage pins the generic strings and rejects issuer, audience, JWT,
and algorithm wording from raw display output.

## Consequences

Authentication and OIDC adapters should continue matching typed variants for
metrics or audit classification. Accidental raw error display no longer exposes
which identity claim failed.
