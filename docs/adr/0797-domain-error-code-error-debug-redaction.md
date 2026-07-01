# ADR 0797: Redact error code error debug output

Status: Accepted

## Context

`ErrorCodeError::TooLong` carries the rejected error-code length. Its derived
`Debug` formatter exposed that validation detail to diagnostics and containing
debug output.

## Decision

Use a custom `Debug` implementation for `ErrorCodeError` that preserves failure
shape while redacting rejected lengths.

## Consequences

Diagnostics can still distinguish empty, malformed, empty-segment, and overlong
error codes, but raw validation lengths no longer appear through
`ErrorCodeError` debug output.
