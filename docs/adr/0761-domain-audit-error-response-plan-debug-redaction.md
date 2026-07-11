# ADR 0761: Redact domain audit error-response plan debug output

## Status

Accepted

## Context

`AuditErrorResponsePlan` is the domain boundary for audit client-visible error
status, code, and message values. It is intentionally low-cardinality today,
but new fields added later could carry tenant, resource, or storage detail.

## Decision

Implement custom `Debug` for `AuditErrorResponsePlan`. Keep only the existing
`status`, `code`, and `message` fields in formatter output.

## Consequences

Diagnostics keep the stable client-facing error envelope. Future response-plan
fields must make redaction decisions explicitly instead of inheriting derived
`Debug`.
