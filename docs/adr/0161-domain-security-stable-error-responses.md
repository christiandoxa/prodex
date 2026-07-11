# ADR 0161: Domain security stable error responses

## Status

Accepted

## Context

The domain security boundary models tenant resolution, tenant access,
credential-scope checks, minimum role checks, resource-action authorization, and
explicit role-claim mapping. Raw security errors can include tenant IDs,
credential scopes, role names, and unmapped role claim values. Those values are
useful for trusted diagnostics and append-only audit records, but client-visible
API responses should remain stable and redacted.

## Decision

Add stable response planners to `prodex-domain`:

- `plan_role_claim_error_response`;
- `plan_tenant_resolution_error_response`;
- `plan_tenant_access_error_response`;
- `plan_domain_authorization_error_response`; and
- `plan_resource_authorization_error_response`.

The planners expose deterministic status/code/message triples without leaking
tenant IDs, role claim values, credential scopes, role names, or resource-action
authorization internals.

## Consequences

Authn, authz, gateway, and control-plane composition roots can preserve raw
security errors for audit and trusted diagnostics while returning stable
machine-readable public errors. Missing or unknown role claims still never map
to Admin.
