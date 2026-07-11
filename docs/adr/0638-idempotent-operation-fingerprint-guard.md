# 0638: Idempotent Operation Fingerprint Guard

## Status

Accepted

## Context

`IdempotentOperation::new` accepted an empty or malformed request fingerprint. A
blank fingerprint can collapse distinct mutating admin requests into the same
replay comparison surface, and whitespace/control/non-ASCII values can bypass
the canonical HTTP fingerprint contract when an adapter or test bypasses the
HTTP helper.

## Decision

`IdempotentOperation::new` now returns
`Result<IdempotentOperation, IdempotentOperationError>` and rejects empty or
whitespace-only request fingerprints as well as whitespace, control characters,
and non-ASCII fingerprint bytes. Control-plane and storage boundaries map the
failure to redacted invalid-request-fingerprint or invalid-row errors.

## Consequences

- Mutating admin idempotency records must carry a non-empty, ASCII-graphic
  request fingerprint.
- Persisted idempotency rows with blank fingerprints fail closed during
  materialization.
- Client-visible errors still hide raw idempotency keys, request fingerprints,
  and tenant identifiers.
