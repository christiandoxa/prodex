# ADR 0256: Application Role-Binding Lifecycle Composition

## Status

Accepted

## Context

Role grants and revocations are control-plane authorization changes. Existing
slices made the operations explicit and added tenant-scoped storage plans, but
adapters still needed to order authorization, immutable audit, and role-binding
mutation persistence manually. That increases the risk of unaudited denials or
authorized mutations that bypass the expected operation/kind pairing.

## Decision

Add `plan_application_role_binding_lifecycle` to `prodex-application`. The
planner accepts a `RoleBindingGrant` or `RoleBindingRevoke` control-plane
action, a matching `RoleBindingMutationCommand`, durable store selection, and
audit digests.

The planner rejects non-role-binding operations, wrong resource kinds,
cross-tenant action/mutation pairs, and grant/revoke mutation-kind mismatches
before selecting storage. It always emits append-only audit storage for the
authorization decision and emits role-binding mutation storage only when the
decision is authorized.

## Consequences

Control-plane adapters can route role-binding lifecycle requests through a
single application boundary. Denied changes remain auditable, successful
changes stay tenant-scoped, and client-visible errors do not expose tenant IDs
or internal operation details.
