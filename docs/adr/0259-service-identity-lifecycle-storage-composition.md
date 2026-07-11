# ADR 0259: Service Identity Lifecycle Storage Composition

## Status

Accepted

## Context

The control plane includes `ServiceIdentityCreate`, but service identity state
did not yet have tenant-scoped durable storage or an application composition
boundary. Without that boundary, adapters could create identities without
append-only audit, or duplicate backend-specific SQL selection.

## Decision

Add `ServiceIdentityCreateCommand` and `plan_service_identity_create` to
`prodex-storage`. PostgreSQL and SQLite now expose driver-free DML planners for
tenant-scoped service identity upserts, while migration DDL stays in explicit
migrator plans.

Add `plan_application_service_identity_lifecycle` to `prodex-application`.
The planner accepts a `ServiceIdentityCreate` control-plane action, a matching
`ServiceIdentityCreateCommand`, durable store selection, and audit digests. It
rejects wrong operations, wrong resource kinds, and cross-tenant action/identity
pairs before selecting storage. It always plans append-only audit storage and
emits service identity storage only for authorized decisions.

## Consequences

Control-plane adapters can create service identities through one
side-effect-free application boundary. Denied attempts remain auditable,
successful creates are tenant-scoped, and client-visible errors do not expose
tenant IDs or identity display names.
