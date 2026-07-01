# ADR 0762: Redact domain audit chain error debug output

## Status

Accepted

## Context

`AuditChainError` reports append-only audit chain verification failures. Chain
verification is adjacent to digest material, tenant-scoped events, and storage
integrity diagnostics.

## Decision

Implement custom `Debug` for `AuditChainError`. Keep only the
`PreviousDigestMismatch` variant name in formatter output.

## Consequences

Diagnostics still show that chain verification failed. Future chain error
variants must make redaction decisions explicitly instead of inheriting derived
`Debug`.
