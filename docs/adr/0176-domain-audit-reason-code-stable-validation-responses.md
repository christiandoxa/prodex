# 0176: Domain Audit Reason Code Stable Validation Responses

## Status

Accepted

## Context

Audit reason codes explain security-sensitive decisions such as denials,
policy failures, and failed mutations. They are useful for incident response
only when stable, low-cardinality, and machine-readable. If adapters copy raw
provider errors, route fragments, user input, or credential material into reason
codes, audit exports and API rejection responses can leak sensitive data.

Existing compatibility paths still store `reason_code` as `Option<String>`, but
new gateway and control-plane boundaries need a validated reason-code type and a
redacted rejection plan.

## Decision

`prodex-domain` owns `AuditReasonCode`, `AuditEvent::new_with_reason_code`, and
`plan_audit_reason_code_error_response`.

Validated audit reason codes are non-empty, at most 128 characters, composed
from lowercase ASCII letters, digits, underscores, and dot-separated non-empty
segments. Only zero-length values are empty; whitespace-only values fail as
invalid characters. Validation failures map to stable status/code/message
response plans that do not echo the rejected reason code, length, route
fragment, tenant identifier, principal identifier, resource identifier, or
credential-like material.

## Consequences

- Gateway and control-plane adapters can reject unstable audit reason codes
  before writing audit events.
- Existing call sites can migrate incrementally from raw `reason_code` strings
  to `AuditReasonCode`.
- Raw invalid reason-code strings remain available only to trusted diagnostics.
