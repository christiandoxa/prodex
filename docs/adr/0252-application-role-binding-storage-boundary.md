# ADR 0252: Application Role-Binding Storage Boundary

## Status

Accepted

## Context

Role-binding authorization and storage contracts now exist, but composition
roots still need a single application-level use-case plan. Without that
boundary, HTTP adapters could select storage backends directly and bypass the
shared tenant-scoped storage contract.

## Decision

Add `plan_application_role_binding_mutation` to `prodex-application`. The plan
accepts a durable store selection and a storage `RoleBindingMutationCommand`,
then selects the matching PostgreSQL or SQLite DML plan. Storage errors are
mapped to a redacted `role_binding_storage_unavailable` response.

## Consequences

Control-plane adapters can route role grant and revoke execution through the
application boundary, keeping authorization, idempotency, audit, and durable
tenant-scoped storage planning as separate explicit steps. Composition roots do
not need to know adapter-specific role-binding SQL details.
