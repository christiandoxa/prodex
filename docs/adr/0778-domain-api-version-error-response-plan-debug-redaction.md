# ADR 0778: Redact API version error-response plan debug output

## Status

Accepted

## Context

`ApiVersionErrorResponsePlan` is the domain boundary for client-visible API
version lifecycle errors. It is intentionally low-cardinality today, but future
fields could carry requested versions, sunset timestamps, or policy detail.

## Decision

Implement custom `Debug` for `ApiVersionErrorResponsePlan`. Keep only the
existing stable client-facing `status`, `code`, and `message` fields in
formatter output.

## Consequences

Diagnostics keep the stable API version error envelope. Future response-plan
fields must make redaction decisions explicitly instead of inheriting derived
`Debug`.
