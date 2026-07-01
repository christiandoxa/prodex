# 0422: Domain trace ID display redaction

## Status

Accepted

## Context

Trace IDs enter from request headers and are high-cardinality operational data.
API responses already redact invalid trace IDs, but generic error display text
may be logged before response planning.

## Decision

`TraceIdError::Display` no longer includes the rejected trace ID length.

## Consequences

- Invalid trace ID metadata stays out of generic error plumbing.
- Error response codes remain unchanged.
