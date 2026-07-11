# ADR 0494: Fail Closed For Invalid OIDC Admin Tokens

## Status

Accepted.

## Context

Native OIDC admin authentication must reject malformed, expired, wrong-issuer, wrong-audience, disallowed-algorithm, unknown-key, or otherwise invalid bearer tokens. Falling back to another admin identity after a failed OIDC parse would allow confused-credential access to control-plane endpoints.

## Decision

OIDC bearer tokens are accepted only after JWT header parsing, allowed algorithm selection, JWKS key lookup, signature validation, issuer validation, audience validation, and expiry validation succeed. Malformed, expired, wrong-issuer, wrong-audience, disallowed-algorithm, and unknown-key tokens are covered by regression tests and resolve to unauthenticated admin requests.

## Consequences

Invalid OIDC tokens fail closed with the existing admin authentication failure response. Operators can still use configured admin tokens or trusted SSO separately; a bad OIDC token does not create admin privileges.
