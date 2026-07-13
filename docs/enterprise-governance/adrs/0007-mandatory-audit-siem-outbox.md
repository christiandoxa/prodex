# ADR 0007: Mandatory Audit and SIEM Outbox

- Status: Proposed
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

Append-only audit, redaction and hash-chain helpers exist. Transactional
governance audit/outbox and production SIEM delivery are not complete.
