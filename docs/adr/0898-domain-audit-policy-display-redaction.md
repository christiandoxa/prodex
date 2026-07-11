# ADR 0898: Domain audit policy display redaction

## Status

Accepted.

## Context

Audit digest, time-range, retention-policy, and reason-code response planners
already expose stable messages. Raw `Display` output still distinguished empty,
too-long, invalid-character, inverted-range, and retention-window validation
classes.

## Decision

Render `AuditDigestError`, `AuditTimeRangeError`,
`AuditRetentionPolicyError`, and `AuditReasonCodeError` with the same messages
used by their response planners. Keep typed variants and response codes
unchanged for trusted classification.

Regression coverage pins exact display strings and rejects length/value leakage
from raw display and debug output.

## Consequences

Audit query, retention, and chain-integrity boundaries can safely fall back to
raw display strings without exposing request validation shape. Diagnostics
should continue matching typed variants for exact reasons.
