# ADR 0825: Redact domain policy snapshot debug output

Status: Accepted

## Context

`PolicySnapshot` and `ValidatedPolicySnapshot` carry revision identifiers,
issued-at timestamps, opaque integrity metadata, and policy payloads. Derived
`Debug` output exposed those values through diagnostics.

## Decision

Use custom `Debug` implementations for policy snapshot wrappers that preserve
snapshot shape while redacting issued-at timestamps and payloads and relying on
redacted debug formatters for revision IDs, digests, and signatures.

## Consequences

Diagnostics can still distinguish raw and validated policy snapshots, but
policy payloads and integrity metadata no longer appear through snapshot debug
output.
