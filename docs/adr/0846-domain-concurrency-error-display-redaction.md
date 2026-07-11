# ADR 0846: Redact domain concurrency error display output

Status: Accepted

## Context

`ConcurrencyError::VersionMismatch` carries expected and actual resource
versions for optimistic concurrency checks. Its `Display` output exposed those
version counters through generic error formatting.

## Decision

Keep the structured enum fields for trusted callers, but make
`ConcurrencyError::Display` render a generic version-mismatch message without
expected or actual values.

## Consequences

Mutation precondition handling and response planning are unchanged, while
stringified concurrency errors no longer expose resource version counters.
