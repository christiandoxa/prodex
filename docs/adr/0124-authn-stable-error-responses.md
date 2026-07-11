# ADR 0124: Authentication stable error responses

## Status

Accepted

## Context

The authentication boundary validates OIDC/JWT metadata with cached JWKS and an
explicit role mapper. Authentication failures can include sensitive or
implementation-specific details such as key IDs, issuer/audience mismatches,
JWKS cache state, or role claim internals. Composition roots need a stable
client-visible mapping that does not leak those details.

## Decision

Add `plan_authentication_error_response` to `prodex-authn`. It maps
`AuthenticationError` values into stable status/code/message triples:

- JWKS refresh or cache unavailability becomes
  `authentication_temporarily_unavailable`;
- missing tenant claims become `tenant_required`;
- missing or unknown role claims become `role_not_authorized`; and
- invalid signatures, key IDs, token times, or OIDC claims become
  `invalid_token`.

The raw authentication error remains available to trusted diagnostics outside
the client-visible response envelope.

## Consequences

Adapters can fail closed while avoiding disclosure of key IDs, IdP details, role
mapping internals, or cache state to clients.
