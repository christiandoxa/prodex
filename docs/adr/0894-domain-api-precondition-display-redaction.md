# ADR 0894: Domain API precondition display redaction

## Status

Accepted.

## Context

Resource-version and concurrency-precondition response planners already expose
stable messages. Raw `Display` output still distinguished zero vs overflow
resource versions, named version/entity-tag requirements for missing
preconditions, and used shorter mismatch wording than the response planner.

## Decision

Render `ResourceVersionError` as `resource version is invalid` and render
`ConcurrencyError` variants with the same messages used by
`plan_concurrency_error_response`. Keep typed variants and response codes
unchanged for trusted classification.

Regression coverage pins exact display strings and rejects raw version,
entity-tag, zero, and overflow details from display output.

## Consequences

Mutation precondition boundaries can safely fall back to raw display strings
without exposing token type or version-validation shape. Diagnostics should
continue matching typed variants for exact reasons.
