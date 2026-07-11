# ADR 0793: Redact trace id error debug output

Status: Accepted

## Context

`TraceIdError::TooLong` carries the rejected trace ID length. Its derived
`Debug` formatter exposed that validation detail to diagnostics and containing
debug output.

## Decision

Use a custom `Debug` implementation for `TraceIdError` that preserves failure
shape while redacting rejected lengths.

## Consequences

Diagnostics can still distinguish empty, malformed, and overlong trace IDs, but
raw validation lengths no longer appear through `TraceIdError` debug output.
