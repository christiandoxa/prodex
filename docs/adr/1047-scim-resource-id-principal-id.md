# ADR 1047: SCIM resource IDs use PrincipalId

## Status

Accepted.

## Context

Gateway SCIM user creation previously generated compatibility resource IDs by
reusing virtual-key token entropy and formatting it as `user_<token>`. The value
was globally random enough for compatibility, but it was not one of the typed
enterprise identifiers required by the platform.

Affected symbol:

- `runtime_gateway_generate_scim_user_id`

The risk is identifier drift: new SCIM users can enter control-plane and audit
surfaces with non-domain resource identifiers while durable user lifecycle
planning requires typed `PrincipalId` values elsewhere.

## Decision

New gateway SCIM user resource IDs now use `PrincipalId::new()` and are emitted
as canonical UUIDv7 strings.

## Consequences

Existing SCIM rows remain addressable by their stored IDs. Newly created SCIM
users get typed globally unique IDs that align with durable user lifecycle and
audit identity boundaries.

Regression coverage:

- `generated_scim_user_id_is_typed_principal_id`
