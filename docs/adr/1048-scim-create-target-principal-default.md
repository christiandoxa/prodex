# ADR 1048: SCIM create defaults target principal from resource ID

## Status

Accepted.

## Context

Gateway SCIM creation now generates typed `PrincipalId` resource IDs, while
durable SQLite/PostgreSQL user lifecycle planning requires `user_id` to be a
typed `PrincipalId`. When unscoped admins omitted `user_id`, durable creation
still failed even though the generated SCIM resource ID was already a valid
principal identifier.

Affected symbols:

- `runtime_gateway_admin_scim_create_user_response`
- `runtime_gateway_admin_scim_default_target_user_id`

The risk is durable control-plane drift: new typed SCIM resources cannot enter
the durable user lifecycle unless clients duplicate the same principal ID in a
separate field.

## Decision

When creating a SCIM user, if `user_id` is absent and the admin token does not
supply one, Prodex defaults `user_id` to the generated typed SCIM resource ID.
Explicit request or admin-scope `user_id` values still take precedence.

## Consequences

New SCIM creates can reach durable user lifecycle planning with one canonical
typed principal. Legacy rows and explicit `user_id` requests are unchanged.

Regression coverage:

- `scim_create_defaults_missing_user_id_to_generated_resource_principal_id`
