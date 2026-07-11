# ADR 0783: Redact resource version error debug output

## Status

Accepted

## Context

`ResourceVersionError` represents invalid optimistic-concurrency resource
versions. It is low-cardinality today, but future variants could carry rejected
version values or overflow details.

## Decision

Implement custom `Debug` for `ResourceVersionError`. Keep only the `Zero` and
`Overflow` variant names in formatter output.

## Consequences

Diagnostics still distinguish zero and overflow failures. Future resource
version error variants must make redaction decisions explicitly instead of
inheriting derived `Debug`.
