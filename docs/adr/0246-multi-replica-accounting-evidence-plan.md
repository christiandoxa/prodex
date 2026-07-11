# ADR 0246: Multi-replica accounting evidence plan

## Status

Accepted

## Context

The storage boundary already defines the required multi-replica accounting
concurrency checks: no lost update, no duplicate charge, no dropped ledger
event, no request ID collision, and no undocumented limit overshoot. That spec
describes what must be tested, but production readiness also needs a small
contract for accepting evidence after database-backed tests run.

Without an evidence plan, a deployment adapter could claim that a subset of
checks passed or that fewer replicas were exercised.

## Decision

`prodex-storage` now exposes `MultiReplicaAccountingEvidence` and
`plan_multi_replica_accounting_verification`. Verification accepts evidence only
when:

- the evidence replica count matches the planned spec;
- every required check in the spec appears in the passed checks; and
- the limit-overshoot tolerance is explicitly documented.

Missing checks, replica-count mismatches, and missing overshoot tolerance fail
closed through the same stable multi-replica accounting error response family.

## Consequences

Future database-backed concurrency tests can emit adapter-neutral evidence and
composition roots can decide production readiness from the same storage
boundary. The planner does not run the database test itself; it records the
minimum proof needed before a multi-replica accounting claim is accepted.
