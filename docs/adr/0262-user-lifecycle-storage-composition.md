# ADR 0262: User Lifecycle Storage Composition

## Status

Accepted

## Context

The control plane already models `UserInvite` and SCIM user lifecycle
operations as admin-only, audited mutations. Those operations did not yet have
a durable user registry storage contract. Without a shared storage and
application composition boundary, HTTP adapters could persist users outside the
authorization/audit decision or duplicate backend-specific SQL.

Regulated multi-tenant deployments need user state to be tenant-keyed,
auditable, and soft-deletable while keeping request-serving paths free of DDL.

## Decision

Add `UserLifecycleCommand` and `plan_user_lifecycle` to `prodex-storage`. The
command carries tenant storage key, tenant ID, principal ID, external ID,
display name, lifecycle kind, and operation time. Create/update require a
display name; all kinds require a non-empty external ID.

PostgreSQL and SQLite add `prodex_users` to explicit migration plans and expose
request-path DML plans for user upsert and soft delete. PostgreSQL keeps RLS on
`prodex_users`; SQLite keeps local operations inside immediate transactions.

Add `plan_application_user_lifecycle` to `prodex-application`. The planner
accepts `UserInvite` and `ScimUser*` operations on `ResourceKind::User`,
checks tenant and operation/kind alignment, always plans append-only audit
storage, and only plans user storage after an authorized decision.

## Consequences

User and SCIM lifecycle mutations now share a single authorization, audit, and
storage composition boundary. Client-visible error responses remain redacted
and do not expose tenant IDs, external IDs, display names, or SQL details.
