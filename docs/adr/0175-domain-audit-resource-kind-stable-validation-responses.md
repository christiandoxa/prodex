# 0175: Domain Audit Resource Kind Stable Validation Responses

## Status

Accepted

## Context

Audit resources are queried, exported, and correlated with authorization,
tenant isolation, billing, and incident response workflows. Resource kind names
must stay low-cardinality and machine-readable. If adapters derive them from raw
routes, provider payloads, or copied input, audit records and public rejection
responses can leak path fragments, tenant names, resource identifiers, or
credential-like material.

Existing compatibility paths still use `AuditResource::new`, but new gateway
and control-plane boundaries need a validated constructor and redacted rejection
plan.

## Decision

`prodex-domain` owns `AuditResource::try_new`,
`AuditResource::validate_kind`, and
`plan_audit_resource_kind_error_response`.

Validated audit resource kinds are non-empty, at most 96 characters, composed
from lowercase ASCII letters, digits, underscores, and dot-separated non-empty
segments. Only zero-length values are empty; whitespace-only values fail as
invalid characters. Validation failures map to stable status/code/message
response plans that do not echo the rejected resource kind, length, route
fragment, tenant identifier, principal identifier, resource identifier, or
credential-like material.

## Consequences

- Gateway and control-plane adapters can reject unstable audit resource kinds
  before writing or exposing audit events.
- Existing call sites can migrate incrementally from `AuditResource::new` to
  the validated path.
- Raw invalid resource kind strings remain available only to trusted
  diagnostics.
