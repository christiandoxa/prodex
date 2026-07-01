# ADR 0247: Application runtime accounting evidence boundary

## Status

Accepted

## Context

`prodex-storage` defines the multi-replica accounting concurrency spec and the
evidence verifier. Composition roots should not call storage verification
directly or duplicate the error mapping, because startup/readiness surfaces must
return stable redacted responses.

## Decision

`prodex-application` now exposes
`plan_application_runtime_accounting_verification`. The planner accepts an
`ApplicationRuntimePlan` and `MultiReplicaAccountingEvidence`, requires that the
runtime plan actually enabled multi-replica accounting checks, delegates evidence
verification to `prodex-storage`, and maps failures through a stable
`runtime_accounting_verification_invalid` response.

## Consequences

Production startup/readiness adapters can verify database-backed concurrency
evidence through one application boundary. Optional single-node/local runtime
plans cannot accidentally accept evidence and claim the enterprise accounting
gate passed.
