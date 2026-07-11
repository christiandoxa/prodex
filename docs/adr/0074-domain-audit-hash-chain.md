# ADR 0074: Domain audit hash-chain envelope

## Status

Accepted.

## Context

The enterprise security target requires immutable audit events for
security-sensitive actions. `prodex-domain` already modeled audit events with
canonical principal and tenant context, but it did not include a portable
envelope for append-only chain verification. Storage implementations need a
shared model for previous/current digests without coupling domain code to a
specific hashing algorithm or storage backend.

## Decision

Add `AuditDigest`, `AuditEnvelope`, and `AuditChainError`. An envelope wraps an
`AuditEvent` with an optional previous digest and current event digest.
`verify_chain_link` checks that the persisted previous digest matches the
expected prior event digest. Digest computation and durable append remain the
responsibility of storage/audit-log adapters outside `prodex-domain`.

## Consequences

- Audit stores can provide tamper-evident append-only chains using shared domain
  semantics.
- Domain code still avoids filesystem, database, network, and hashing-library
  dependencies.
- Future audit export/verification commands can validate chain continuity without
  redefining the envelope format.
