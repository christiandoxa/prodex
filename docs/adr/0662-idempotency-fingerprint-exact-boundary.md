# 0662. Idempotency Fingerprint Exact Boundary

## Status

Accepted

## Context

Idempotency keys and request fingerprints are replay-sensitive inputs for
control-plane mutations and accounting flows. The domain boundary validated
printable characters, but empty checks used `trim()`, so whitespace-only values
were classified as empty instead of malformed exact input.

That classification can hide malformed replay metadata in diagnostics and
regression tests.

## Decision

`IdempotencyKey::new` and `IdempotentOperation::new` now treat only the empty
string as empty. Whitespace-only keys or request fingerprints are rejected as
invalid characters.

## Consequences

- Replay metadata is validated exactly as supplied.
- Stable idempotency error responses remain redacted and unchanged.
- Existing valid printable ASCII keys and fingerprints continue to work.
