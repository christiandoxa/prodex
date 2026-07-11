# ADR 0818: Redact idempotency key error debug output

Status: Accepted

## Context

`IdempotencyKeyError` can carry rejected key lengths, character indexes, and
invalid characters. Its derived `Debug` formatter exposed those validation
details through diagnostics.

## Decision

Use a custom `Debug` implementation for `IdempotencyKeyError` that preserves
failure shape while redacting rejected lengths, indexes, and characters.

## Consequences

Diagnostics can still distinguish empty, overlong, and invalid-character
idempotency key failures, but raw validation details no longer appear through
`IdempotencyKeyError` debug output.
