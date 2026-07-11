# ADR 0784: Redact resource version error-response plan debug output

## Status

Accepted

## Context

`ResourceVersionErrorResponsePlan` is the domain boundary for client-visible
resource-version validation failures. It is low-cardinality today, but future
fields could carry rejected version values or overflow details.

## Decision

Implement custom `Debug` for `ResourceVersionErrorResponsePlan`. Keep only the
existing stable client-facing `status`, `code`, and `message` fields in
formatter output.

## Consequences

Diagnostics keep the stable resource-version error envelope. Future
response-plan fields must make redaction decisions explicitly instead of
inheriting derived `Debug`.
