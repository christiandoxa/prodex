# ADR 0427: Redact audit policy validation display values

## Status

Accepted.

## Context

Audit query and retention validation errors carried rejected numeric inputs in
their `Display` text. Domain callers already expose stable audit error response
plans, so raw rejected values are not required for public messages.

## Decision

`AuditTimestampError`, `AuditPageLimitError`,
`AuditRetentionPolicyError`, and `AuditRetentionBatchLimitError` now keep
structured values for matching and tests, but their `Display` output omits the
rejected input.

## Consequences

Domain error display text is safer for logs and API adapters. Callers that need
exact rejected values must inspect the typed error variant directly.
