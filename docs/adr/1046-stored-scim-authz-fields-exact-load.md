# ADR 1046: Stored SCIM authorization fields load exactly

## Status

Accepted.

## Context

Gateway SSO/OIDC authentication can inherit role, tenant, governance scopes, and
key-prefix scopes from persisted SCIM users. Admin SCIM writes already reject
whitespace-bearing `userName`, role, governance scopes, and key-prefix scopes,
but persisted compatibility rows can still be manually corrupted.

Affected symbols:

- `runtime_gateway_scim_user_by_name`
- `runtime_gateway_scim_user_auth_entry_from_stored`

The risk is authorization drift: a corrupt SCIM row could supply malformed
tenant or key-prefix scopes to admin authentication even though the control
plane API would reject those values.

## Decision

SCIM users used for admin authentication must have exact non-empty `id` and
`userName` values, and exact optional role, governance scope, and key-prefix
scope values without whitespace. Empty optional scopes are treated as absent for
authentication.

## Consequences

Canonical SCIM users keep working. Corrupt persisted SCIM users are ignored by
the SSO/OIDC auth lookup instead of contributing malformed authorization scope.

Regression coverage:

- `stored_scim_user_auth_entry_rejects_padded_authz_fields`
- `stored_scim_user_auth_entry_normalizes_empty_optional_scope_absent`
