# ADR 0848: Redact domain ID parse display kind

Status: Accepted

## Context

`IdParseError` keeps the identifier kind so response planning can still emit
typed machine-readable codes. Its `Display` output included that kind, which can
reveal internal identifier topology through generic error formatting.

## Decision

Keep `IdParseError::kind()` for trusted response planning, but render
`Display` as a generic identifier validation failure.

## Consequences

Malformed identifier values and identifier kinds stay out of stringified
errors, while response planners still produce typed error codes.
