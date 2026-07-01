# 0420: Domain role claim redaction

## Status

Accepted

## Context

Unknown OIDC role claims are denied, but error values may be logged by callers
before being converted into the redacted API error envelope. Raw role claim
values can contain internal IdP group names or attacker-controlled text.

## Decision

`RoleClaimError::Unknown` no longer stores or displays the unknown role claim
value.

## Consequences

- Security logs and generic error plumbing avoid leaking raw role claims through
  `Display` or `Debug`.
- API responses remain unchanged and redacted.
