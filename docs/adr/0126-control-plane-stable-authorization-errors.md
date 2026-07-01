# ADR 0126: Control-plane stable authorization errors

## Status

Accepted

## Context

The control-plane boundary authorizes administrative tenant, identity, virtual
key, policy, billing, audit, configuration, and break-glass operations. Raw
control-plane authorization errors include expected credential scope, actual
role, operation kind, resource kind, tenant mismatch state, and break-glass
timestamps. Those values are important for trusted audit diagnostics but should
not be exposed in client-visible API responses.

## Decision

Add `plan_control_plane_authorization_error_response` to
`prodex-control-plane`. It maps `ControlPlaneAuthorizationError` values into
stable forbidden response plans:

- credential-scope violations become `credential_scope_not_allowed`;
- role failures become `role_not_authorized`;
- missing or cross-tenant access becomes `tenant_access_denied`;
- operation/resource mismatches become `resource_not_authorized`; and
- expired or missing break-glass authorization becomes
  `break_glass_not_authorized`.

The raw error remains available for append-only audit events and trusted,
redacted diagnostics.

## Consequences

Control-plane HTTP adapters can return deterministic machine-readable errors
without leaking role names, scope internals, operation/resource topology, tenant
identifiers, or break-glass timing details.
