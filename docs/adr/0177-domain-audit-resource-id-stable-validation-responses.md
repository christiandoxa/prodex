# 0177: Domain Audit Resource ID Stable Validation Responses

## Status

Accepted

## Context

Audit resource identifiers connect immutable audit events to tenants,
principals, virtual keys, policy revisions, ledgers, and operational resources.
They can also be sensitive when copied from paths, provider payloads, account
identifiers, or user input. Regulated deployments need a validated path for
resource IDs and a redacted rejection plan before dynamic values enter audit
events or client-visible error envelopes.

Existing compatibility paths still store `AuditResource.id` as `Option<String>`,
but new gateway and control-plane boundaries need a validated resource-ID type.

## Decision

`prodex-domain` owns `AuditResourceId`,
`AuditResource::new_with_resource_id`, and
`plan_audit_resource_id_error_response`.

Validated audit resource IDs are non-empty, at most 256 characters, and limited
to lowercase ASCII letters, digits, dots, underscores, hyphens, and colons.
Only zero-length values are empty; whitespace-only values fail as invalid
characters. Validation failures map to stable status/code/message response plans
that do not echo the rejected resource ID, length, route fragment, tenant
identifier, principal identifier, credential-like material, or provider account
material.

## Consequences

- Gateway and control-plane adapters can reject unstable audit resource IDs
  before writing audit events.
- Existing call sites can migrate incrementally from raw resource-ID strings to
  `AuditResourceId`.
- Raw invalid resource IDs remain available only to trusted diagnostics.
