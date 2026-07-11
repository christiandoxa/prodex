# ADR 0713: Runtime Request IDs Use Per-Request Entropy

## Status

Accepted

## Context

The runtime proxy carries an internal `u64` request identifier through logging,
dispatch, websocket, and local rewrite paths. The startup seed already reserved
high bits for a runtime instance, but subsequent IDs still advanced from a
process-local `AtomicU64` value. That leaves the internal value weaker than the
enterprise requirement for multi-replica-safe request identity.

## Decision

Generate the high bits of each runtime request identifier from fresh
`RequestId` UUIDv7 entropy. Keep the existing `AtomicU64` sequence only as a
low-bit local suffix so request logs remain compact and the internal `u64`
contract does not change.

## Consequences

- Runtime request IDs no longer depend only on a process-local counter after
  startup.
- Existing call sites can keep the compact `u64` request field while external
  API, ledger, and audit surfaces continue using typed UUIDv7 identifiers.
- The low-bit sequence remains a local diagnostic suffix, not a global identity
  source.
