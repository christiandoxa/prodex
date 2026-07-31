# ADR 0003: Policy Approval, Activation, and LKG

- Status: Accepted
- Scope: target control plane

## Context

Mutable policy files and a direct active pointer do not provide maker-checker
separation, immutable evidence, safe concurrent activation or auditable
rollback.

## Decision

Store mutable drafts separately from immutable, checksum-addressed revisions.
Submission freezes and validates the candidate. Independent approval binds
tenant, revision/checksum, policy kind, approver role, quorum and expiry; the
maker cannot count as checker. Activation atomically updates active/LKG history
with audit and SIEM-outbox records under optimistic concurrency. PostgreSQL
pointer changes also append a transactional invalidation-outbox event and queue
a bounded cache wake-up. Gateways replay unacknowledged events, refresh from the
authoritative pointers before acknowledgement, and retain a current head for
restarted replicas. Rollback activates a previous immutable approved revision;
invalidated/revoked revisions remain ineligible. Gateways reject unknown
mandatory schema semantics.

## Consequences

The database is authoritative; caches are rebuildable. Availability may use a
verified LKG only when mode/policy permits and invalidation has not occurred.
Every race, rejection, activation, rollback and failed attempt is audited.

## Current implementation

Prodex implements immutable signed governance revisions, approval transitions,
maker-checker/quorum enforcement, optimistic activation, active/LKG pointers,
rollback, audit and SIEM-outbox contracts, PostgreSQL outbox replay, and
notification wake-ups. PostgreSQL cache fanout now uses a durable
tenant-scoped invalidation outbox with per-replica acknowledgement; `NOTIFY`
remains a non-authoritative wake-up only.
Evidence includes
`maker_checker_quorum_and_activation_are_enforced`,
`gateway_policy_http_revocation_invalidates_cache_and_lkg`, the
SQLite governance repository lifecycle suite, and the live PostgreSQL all-kind
lifecycle proof.
