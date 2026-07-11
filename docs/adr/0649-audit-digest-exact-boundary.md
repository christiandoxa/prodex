# 0649: Audit Digest Exact Boundary

## Status

Accepted

## Context

`AuditDigest::new` trimmed input before validation and storage. Audit digests
are hash-chain metadata, so silently normalizing input could hide malformed
transport, storage, or adapter behavior.

## Decision

`AuditDigest::new` now validates and stores the exact provided value. Whitespace
only is an invalid digest. Only zero-length values are empty digest errors.

## Consequences

- Audit hash-chain metadata is exact at the domain boundary.
- Existing valid digest values keep working.
- Stable audit error responses continue to redact rejected digest values and
  lengths.
