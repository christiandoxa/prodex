# ADR 0930: Storage role-binding mutation debug redaction

## Status

Accepted.

## Context

Role-binding mutation commands and plans carry tenant IDs, role-binding IDs,
principal IDs, roles, mutation kinds, and timestamps. IDs and timestamps are
needed by storage adapters but should not appear in generic debug output.

## Decision

Use custom `Debug` implementations for `RoleBindingMutationCommand` and
`RoleBindingMutationPlan`. Redact storage keys, tenant IDs, role-binding IDs,
principal IDs, and timestamps while preserving low-cardinality role and mutation
kind.

Regression coverage rejects tenant, role-binding, principal, and timestamp
values in rendered role-binding mutation debug output.

## Consequences

Storage diagnostics can identify role-binding mutation shape without exposing
principal or tenant identifiers. Mutation planning behavior is unchanged.
