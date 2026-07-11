# ADR 0920: Storage multi-replica accounting display redaction

## Status

Accepted.

## Context

Multi-replica accounting verification errors carry gateway replica counts,
storage topology, and required evidence checks. The response planner already
returns a stable redacted message, but raw `Display` formatting exposed those
deployment details.

## Decision

Use the same stable message for every
`MultiReplicaAccountingConcurrencySpecError` display string. Keep the structured
variants for trusted diagnostics, verification logic, and tests.

Regression coverage verifies that replica counts are not present in stringified
multi-replica accounting errors.

## Consequences

Operator-facing or accidental stringified storage errors no longer expose
replica-count, topology, or evidence-check details. Planning and verification
behavior remains unchanged.
