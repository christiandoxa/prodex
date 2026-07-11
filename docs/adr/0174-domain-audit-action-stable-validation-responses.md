# 0174: Domain Audit Action Stable Validation Responses

## Status

Accepted

## Context

Regulated deployments rely on immutable audit events for security-sensitive
actions. Audit action names are indexed, queried, exported, and correlated with
authorization decisions. If an adapter builds action names from raw routes,
provider messages, or copied input, action fields can become unstable,
high-cardinality, or contain credential-like material.

Existing compatibility paths still need `AuditAction::new`, but new gateway and
control-plane boundaries need a validated constructor and a redacted rejection
plan.

## Decision

`prodex-domain` owns `AuditAction::try_new`, `AuditAction::validate`, and
`plan_audit_action_error_response`.

Validated audit actions are non-empty, at most 128 characters, composed from
lowercase ASCII letters, digits, underscores, and dot-separated non-empty
segments. Only zero-length values are empty; whitespace-only values fail as
invalid characters. Validation failures map to stable status/code/message
response plans that do not echo the rejected action, length, route fragment,
tenant identifier, principal identifier, resource identifier, or
credential-like material.

## Consequences

- Gateway and control-plane adapters can reject unstable audit action names
  before writing or exposing audit events.
- Existing call sites can migrate incrementally from `AuditAction::new` to the
  validated path.
- Raw invalid action strings remain available only to trusted diagnostics.
