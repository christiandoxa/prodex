# ADR 0730: Redact domain audit envelope debug output

## Status

Accepted

## Context

Audit envelopes carry immutable audit events plus append-only hash-chain digest
material. Exact event and digest values are required for storage and chain
verification, but generic `Debug` output can appear in panic diagnostics or
structured logs outside authorized audit tooling.

## Decision

Implement custom `Debug` for `AuditDigest` and `AuditEnvelope`. Digest debug
output is fully redacted. Envelope debug output delegates to the already redacted
event debug shape and redacts previous/current digest presence and values.
Serialization, equality, and chain-link verification are unchanged.

## Consequences

- Append-only audit chain verification remains exact.
- Generic diagnostics no longer expose digest material, event identifiers,
  tenant IDs, principal IDs, or resource IDs through envelope dumps.
- Exact chain material remains available through authorized audit storage and
  verification tooling.
