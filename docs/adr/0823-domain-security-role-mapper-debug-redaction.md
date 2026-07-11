# ADR 0823: Redact domain security role mapper debug output

Status: Accepted

## Context

`ExplicitRoleMapper` stores configured role-claim strings and their mapped
domain roles. Derived `Debug` output exposed those configured identity-provider
claim values through diagnostics.

## Decision

Use a custom `Debug` implementation for `ExplicitRoleMapper` that redacts the
mapping table.

## Consequences

Diagnostics can still identify the role mapper type, but configured role-claim
values and mapped roles no longer appear through `ExplicitRoleMapper` debug
output.
