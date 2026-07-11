# 0425: Domain idempotency key display redaction

## Status

Accepted

## Context

Idempotency keys enter through HTTP request metadata and can contain
attacker-controlled text. API responses already redact invalid keys, but generic
error display text can be logged before response planning.

## Decision

`IdempotencyKeyError::Display` no longer includes rejected key lengths,
characters, or byte indexes.

## Consequences

- Invalid idempotency-key details stay out of generic error plumbing.
- Error response codes and messages remain unchanged.
