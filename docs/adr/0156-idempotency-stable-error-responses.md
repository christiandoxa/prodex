# ADR 0156: Idempotency stable error responses

## Status

Accepted

## Context

The domain idempotency boundary validates replay keys and rejects unsafe replay
attempts for mutating control-plane and billing operations. Raw
`IdempotencyKeyError` values can include token lengths, invalid characters, and
byte indexes. Raw `IdempotencyConflict` values distinguish tenant mismatches,
defensive key mismatches, and request fingerprint mismatches. These details are
useful for trusted diagnostics and audit classification, but client-visible API
responses should not leak replay token metadata, tenant-affinity internals, or
request fingerprints.

## Decision

Add stable response planners to `prodex-domain`:

- `plan_idempotency_key_error_response` maps every malformed idempotency key to
  `idempotency_key_invalid` with a bad-request status;
- `plan_idempotency_conflict_response` maps tenant or defensive key mismatch to
  `idempotency_replay_conflict` with a conflict status; and
- `plan_idempotency_conflict_response` maps request fingerprint mismatch to
  `idempotency_key_reused` with a conflict status.

The response messages stay generic and exclude raw keys, token lengths,
characters, byte indexes, tenant IDs, and request fingerprints.

## Consequences

Gateway and control-plane composition roots can expose deterministic
machine-readable idempotency failures while retaining raw idempotency errors for
trusted diagnostics, append-only audit records, and replay investigation.
