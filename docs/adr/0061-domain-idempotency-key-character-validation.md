# ADR 0061: Restrict domain idempotency keys to printable ASCII tokens

## Status

Accepted

## Context

The enterprise target requires replay-safe idempotency for mutating control-plane
operations and billing/accounting flows. Idempotency keys are commonly supplied
through HTTP headers, persisted as lookup keys, and may appear in diagnostics. If
keys contain whitespace, control characters, or non-ASCII text, they can become
ambiguous across transports, enable header/log injection, or produce inconsistent
storage keys across adapters.

## Decision

The pure domain `IdempotencyKey` constructor now accepts only trimmed, non-empty,
printable ASCII graphic characters with the existing 256-byte limit. Invalid
characters return `IdempotencyKeyError::InvalidCharacter { index, character }`.

## Consequences

- Domain-level idempotency keys are single-line transport-safe tokens before they
  reach HTTP/storage adapters.
- Existing empty and over-length validation remains unchanged.
- Regression tests cover spaces, newlines, and non-ASCII characters.
