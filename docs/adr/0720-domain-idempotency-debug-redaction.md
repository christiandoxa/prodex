# ADR 0720: Redact domain idempotency debug output

## Status

Accepted

## Context

Idempotency keys, request fingerprints, tenant IDs, and stored replay responses
are replay-sensitive control-plane data. Stable client error planners already
hide those values, but derived `Debug` output for domain idempotency state could
still print them in panic diagnostics or structured logs.

## Decision

Implement custom `Debug` for `IdempotencyKey`, `IdempotentOperation`,
`IdempotencyRecord`, `IdempotencyEntry`, `IdempotencyDecision`, and
`IdempotencyReplayDecision`. Debug output keeps the variant/state shape needed
for tests and operator triage while replacing tenant IDs, keys, request
fingerprints, and replay responses with redacted placeholders.

## Consequences

- Replay semantics, serialization, equality, and storage contracts are
  unchanged.
- Panic/log diagnostics retain enough state to distinguish pending, completed,
  execute, and replay decisions.
- Sensitive replay material is less likely to leak through generic debug
  logging.
