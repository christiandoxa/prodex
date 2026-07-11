# ADR 0899: Domain boundary token display redaction

## Status

Accepted.

## Context

Error codes, idempotency keys, and trace IDs enter through request metadata.
Their response planners already expose stable messages, but raw `Display`
output still distinguished empty, too-long, invalid-character, and segment
validation classes.

## Decision

Render `ErrorCodeError`, `IdempotencyKeyError`, and `TraceIdError` with the
same messages used by their response planners. Keep typed variants and response
codes unchanged for trusted classification.

Regression coverage pins exact display strings and keeps existing debug
redaction assertions for rejected lengths, byte indexes, and characters.

## Consequences

Control-plane and observability boundaries can safely fall back to raw display
strings without exposing request metadata validation shape. Diagnostics should
continue matching typed variants for exact reasons.
