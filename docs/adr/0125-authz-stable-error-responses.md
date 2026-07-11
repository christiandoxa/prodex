# ADR 0125: Authorization stable error responses

## Status

Accepted

## Context

The authorization boundary rejects data-plane, control-plane, break-glass, and
tenant access violations before provider or storage side effects occur. Raw
boundary errors intentionally retain diagnostic detail such as expected scopes,
actual roles, boundary kind, and tenant mismatch state. Those details are useful
inside trusted logs and audits but should not become client-visible API text.

## Decision

Add `plan_authorization_error_response` to `prodex-authz`. It maps
`BoundaryAuthorizationError` values into stable forbidden responses:

- credential-scope violations become `credential_scope_not_allowed`;
- role failures become `role_not_authorized`; and
- missing or cross-tenant access becomes `tenant_access_denied`.

The response plan exposes only status, code, and a generic message. Composition
roots may still log or audit the raw authorization error through trusted,
redacted channels.

## Consequences

Data-plane and control-plane adapters can deny authorization consistently without
leaking role names, boundary names, credential-scope internals, tenant IDs, or
resource ownership details to callers.
