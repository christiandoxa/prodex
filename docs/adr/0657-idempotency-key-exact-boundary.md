# 0657: Idempotency Key Exact Boundary

## Status

Accepted

## Context

`IdempotencyKey::new` trimmed input before validation and storage. Idempotency
keys are replay guards for mutating operations, so silently accepting padded
values can hide malformed clients and collapse distinct raw inputs into one
durable replay key.

## Decision

`IdempotencyKey::new` now validates and stores the exact provided value.
Whitespace-only values remain empty errors, while leading or trailing whitespace
is rejected as an invalid character.

## Consequences

- Replay keys stay exact at the domain boundary.
- Existing valid idempotency keys keep working.
- Stable idempotency error responses continue to redact rejected raw values.
