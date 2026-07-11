# 0629: Domain OIDC Audience Character Guard

## Status

Accepted

## Context

OIDC validation policy compares token audiences against configured audience
values. The domain boundary rejected empty audiences, but still accepted values
with whitespace, control characters, or non-ASCII text. Those values are poor
transport tokens and can create ambiguous configuration or logs.

## Decision

`Audience::new` now accepts only non-empty printable ASCII values after trim.
Invalid audience shape maps to the existing redacted
`identity_audience_invalid` response plan.

## Consequences

- Audience allowlists are stable transport-safe tokens before token validation.
- Malformed audience configuration fails before request authentication.
- Existing redacted identity error envelopes remain stable.
