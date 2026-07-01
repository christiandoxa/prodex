# 0185: Domain Audit Query Scope Stable Validation Responses

## Status

Accepted

## Context

Audit query and export paths are tenant-owned. Control-plane adapters must not
return audit events from another tenant when building filtered results or
exports. If tenant scoping is enforced only in storage or HTTP code, different
adapters can drift and client-visible errors can leak tenant identifiers or
event metadata.

## Decision

`prodex-domain` owns `AuditQueryScope` and
`plan_audit_query_scope_error_response`.

`AuditQueryScope::tenant` derives a tenant-scoped query contract from canonical
`TenantContext`. `authorize_event` rejects events whose `tenant_id` does not
match the query scope. Scope failures map to stable status/code/message response
plans that do not echo tenant identifiers, principal identifiers, resource
identifiers, audit event identifiers, or audit payload material.

## Consequences

- Gateway, control-plane, and storage adapters can share one tenant-scope check
  for audit query/export results.
- Cross-tenant audit query failures fail closed before serialization.
- Raw mismatched IDs remain available only to trusted diagnostics.
