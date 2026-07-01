# ADR 0901: Domain idempotency fingerprint display redaction

## Status

Accepted.

## Context

Idempotent operation request fingerprints are replay-sensitive request metadata.
`IdempotentOperationError` debug output already redacts rejected indexes and
characters, but raw `Display` output still distinguished empty fingerprints
from invalid-character fingerprints.

## Decision

Render `IdempotentOperationError` with one stable message:
`request fingerprint is invalid`. Keep typed variants unchanged for trusted
classification.

Regression coverage pins the exact display string and keeps existing debug
redaction assertions for rejected indexes and characters.

## Consequences

Control-plane idempotency boundaries can safely fall back to raw display
strings without exposing request-fingerprint validation shape. Diagnostics
should continue matching typed variants for exact reasons.
