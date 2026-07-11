# ADR 0317: Multi-Replica Accounting Evidence Topology

## Status

Accepted.

## Context

The multi-replica accounting gate requires PostgreSQL as the durable source of
truth and Redis for rebuildable coordination/cache behavior. The existing
evidence verifier checked replica count, required concurrency outcomes, and
documented overshoot tolerance, but the evidence object did not carry the
storage topology it was measured against. A readiness adapter could therefore
accidentally attach evidence from a different topology.

## Decision

`MultiReplicaAccountingEvidence` now includes the `StorageTopology` used by the
concurrency test. `plan_multi_replica_accounting_verification` rejects evidence
whose topology differs from the planned `MultiReplicaAccountingConcurrencySpec`
before accepting replica count or check-list results. The accepted verification
plan also records the verified topology.

`ci:storage-boundary-guard` checks that topology-bearing evidence, topology
mismatch rejection, and topology propagation remain present in
`prodex-storage`.

## Consequences

Production readiness can only claim the multi-replica accounting gate when the
database-backed evidence matches the runtime topology. Evidence from a local,
SQLite, Redis-less, or otherwise mismatched environment cannot satisfy the
enterprise accounting requirement by matching only replica count and checklist
names.
