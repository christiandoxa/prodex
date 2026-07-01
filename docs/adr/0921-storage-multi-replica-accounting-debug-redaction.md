# ADR 0921: Storage multi-replica accounting debug redaction

## Status

Accepted.

## Context

Multi-replica accounting spec, evidence, and verification plans carry storage
topology, gateway replica counts, and required check lists. Those fields are
needed for verification but should not appear in generic debug output.

## Decision

Use custom `Debug` implementations for
`MultiReplicaAccountingConcurrencySpec`, `MultiReplicaAccountingEvidence`, and
`MultiReplicaAccountingVerificationPlan`. Redact topology, replica-count, and
check-list details while leaving typed fields available to callers.

Regression coverage rejects PostgreSQL/Redis topology strings, checklist names,
and exact replica-count formatting in rendered debug output.

## Consequences

Storage diagnostics can identify the accounting structure without exposing
deployment topology. Multi-replica verification behavior remains unchanged.
