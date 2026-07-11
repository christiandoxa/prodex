# ADR 0780: Redact cursor error-response plan debug output

## Status

Accepted

## Context

`CursorErrorResponsePlan` is the domain boundary for client-visible pagination
cursor validation failures. It is low-cardinality today, but future fields could
carry cursor parser detail or request input.

## Decision

Implement custom `Debug` for `CursorErrorResponsePlan`. Keep only the existing
stable client-facing `status`, `code`, and `message` fields in formatter output.

## Consequences

Diagnostics keep the stable pagination cursor error envelope. Future
response-plan fields must make redaction decisions explicitly instead of
inheriting derived `Debug`.
