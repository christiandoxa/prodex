# ADR 0782: Redact entity tag error-response plan debug output

## Status

Accepted

## Context

`EntityTagErrorResponsePlan` is the domain boundary for client-visible
optimistic-concurrency token validation failures. It is low-cardinality today,
but future fields could carry rejected token values or parser details.

## Decision

Implement custom `Debug` for `EntityTagErrorResponsePlan`. Keep only the
existing stable client-facing `status`, `code`, and `message` fields in
formatter output.

## Consequences

Diagnostics keep the stable entity tag error envelope. Future response-plan
fields must make redaction decisions explicitly instead of inheriting derived
`Debug`.
