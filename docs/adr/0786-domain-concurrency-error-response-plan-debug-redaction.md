# ADR 0786: Redact concurrency error-response plan debug output

## Status

Accepted

## Context

`ConcurrencyErrorResponsePlan` is the domain boundary for client-visible
mutation precondition failures. It is low-cardinality today, but future fields
could carry resource versions, entity tags, or request precondition metadata.

## Decision

Implement custom `Debug` for `ConcurrencyErrorResponsePlan`. Keep only the
existing stable client-facing `status`, `code`, and `message` fields in
formatter output.

## Consequences

Diagnostics keep the stable concurrency error envelope. Future response-plan
fields must make redaction decisions explicitly instead of inheriting derived
`Debug`.
