# ADR 0667: Gateway Admin Idempotency Exact Boundary

## Status

Accepted.

## Context

Legacy gateway admin mutations accept an optional `Idempotency-Key` header to
reject duplicate replay. The enterprise domain idempotency boundary already
validates exact replay keys, but the legacy admin adapter trimmed header values
before validation and allowed spaces.

## Decision

The legacy gateway admin adapter now validates the exact `Idempotency-Key`
header value. Present values must be non-empty ASCII graphic strings no longer
than 200 bytes. Padded, whitespace-containing, control-character, or non-ASCII
values fail before they can become replay cache keys.

## Consequences

Malformed idempotency keys use the existing redacted
`invalid_idempotency_key` response and audit denial reason. Missing headers
remain compatible with legacy clients that do not request replay protection.
