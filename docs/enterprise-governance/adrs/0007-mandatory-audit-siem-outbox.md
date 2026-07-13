# ADR 0007: Mandatory Audit and SIEM Outbox

- Status: Accepted
- Scope: governance mutations and policy-selected high-risk dispatches

## Context

Best-effort logs can lose evidence, while synchronous SIEM calls make the hot
path depend on an external sink. Hash chains alone do not make business state
and audit atomic.

## Decision

Use a versioned content-free audit schema with per-tenant/partition hash chains.
Control-plane mutations commit business state, audit event and SIEM outbox row
in one database transaction. Policy-selected high-risk dispatches durably append
the required precommit event before upstream commitment. Bounded asynchronous
workers claim outbox leases, deliver idempotently by event ID, retry with jitter
and expose dead-letter/backlog state. Mandatory operations fail closed if their
audit transaction cannot commit; SIEM itself is never called synchronously from
the request path.

## Consequences

Storage capacity/backlog is a safety signal requiring admission and alerting.
Hash verification, independent export, retention, backup and database/RLS
controls remain necessary. Events exclude bodies, secrets and raw identifiers.

## Implementation status

The candidate implements typed content-free audit/outbox contracts, SQLite and
PostgreSQL repository ports, bounded exporter/lease/dead-letter behavior, and a
mandatory precommit data-plane audit. Evidence includes
`audit_failure_rolls_back_activation_fail_closed`,
`outbox_retries_with_stable_id_then_dead_letters`,
`audit_integrity_health_detects_missing_same_tenant_parent`, and the live
PostgreSQL governance/outbox proof. Mandatory data-plane audit uses a bounded
background writer but waits for transaction acknowledgement before dispatch;
this preserves fail-closed durability and couples admission latency to the
authoritative store. Remote SIEM delivery remains asynchronous.
