# ADR 0669: SCIM Principal and Role Exact Boundary

## Status

Accepted.

## Context

Legacy gateway SCIM provisioning accepted `userName` and `role` strings after
trimming leading or trailing whitespace. Those fields bind provisioned
principals and admin roles. Normalizing request-controlled values can turn a
padded principal or role into an authorized identity.

## Decision

SCIM `userName` and `role` must be exact non-empty strings without whitespace.
Display-oriented SCIM strings such as `displayName` and `externalId` keep their
existing compatibility normalization.

## Consequences

Padded SCIM principals and roles fail with `invalid_scim_field` instead of
being normalized. Canonical SCIM clients that already send exact values are
unchanged.
