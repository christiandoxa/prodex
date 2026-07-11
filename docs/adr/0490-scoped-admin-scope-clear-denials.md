# ADR 0490: Deny Scoped Admin Scope-Clearing Mutations

## Status

Accepted.

## Context

Tenant and governance-scoped gateway admins must not turn scoped resources into global resources. A mutation such as `{"tenant_id": null}` or `{"team_id": null}` is a privilege escalation when issued by a scoped admin.

## Decision

Gateway admin authorization treats explicit `null` scope fields as a requested unscoped value before applying the mutation. Scoped admins therefore fail the same scope check used for cross-tenant or cross-team mutations.

## Consequences

Scoped admins can still mutate resources inside their own scope, but cannot clear the scope boundary. Unscoped admins keep the existing ability to clear optional scope fields.
