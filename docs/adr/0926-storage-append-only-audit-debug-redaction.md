# ADR 0926: Storage append-only audit debug redaction

## Status

Accepted.

## Context

Append-only audit commands and plans carry tenant-scoped storage keys, audit
events, digest-chain inputs, and audit envelopes. These values are needed by
storage adapters, but generic debug output should not expose audit payloads,
resource identifiers, tenant IDs, or digest material.

## Decision

Use custom `Debug` implementations for `AppendOnlyAuditCommand` and
`AppendOnlyAuditPlan`. Redact storage keys, events, digest values, and envelopes
while preserving the low-cardinality append mode.

Regression coverage rejects tenant ID, digest values, and audit resource IDs in
rendered append-only audit debug output.

## Consequences

Storage diagnostics can identify append-only audit command/plan shape without
exposing audit payload or hash-chain material. Audit append behavior is
unchanged.
