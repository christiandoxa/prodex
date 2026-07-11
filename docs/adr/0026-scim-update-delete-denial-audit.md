# ADR 0026: Audit SCIM Update and Delete Authorization Denials

## Status

Accepted

## Context

Gateway admin SCIM create/get scope denials were audited, but update and delete had additional denial paths inside state-store mutation closures. The delete path also preserved an out-of-scope user and surfaced the denial as not found, which made it harder for operators to distinguish authorization failures from absent resources in the audit trail.

## Decision

SCIM update and delete scope denials now append sanitized `gateway_admin` audit events with:

- `action: authorization_denied`
- `outcome: failure`
- `reason: scope_forbidden`
- `resource: scim_user`
- operation-specific action names such as `update_scim_user` and `delete_scim_user`
- the SCIM resource ID, role, actor name, and state backend label

The delete path now returns the same 403 scope-forbidden response shape used by other out-of-scope admin SCIM operations.

## Consequences

Operators can audit denied SCIM lifecycle attempts consistently across create, get, update, and delete operations. No bearer tokens, generated virtual keys, provider credentials, or request bodies are written to the audit payload.
