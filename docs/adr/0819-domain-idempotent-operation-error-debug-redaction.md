# ADR 0819: Redact idempotent operation error debug output

Status: Accepted

## Context

`IdempotentOperationError::InvalidRequestFingerprintCharacter` carries rejected
fingerprint indexes and characters. Its derived `Debug` formatter exposed those
validation details through diagnostics.

## Decision

Use a custom `Debug` implementation for `IdempotentOperationError` that
preserves failure shape while redacting rejected indexes and characters.

## Consequences

Diagnostics can still distinguish empty fingerprint from invalid-character
failures, but raw request-fingerprint validation details no longer appear
through `IdempotentOperationError` debug output.
