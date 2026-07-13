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
with audit and outbox records under optimistic concurrency. Rollback activates
a previous immutable approved revision; invalidated/revoked revisions remain
ineligible. Gateways acknowledge revision changes and reject unknown mandatory
schema semantics.

## Consequences

The database is authoritative; caches are rebuildable. Availability may use a
verified LKG only when mode/policy permits and invalidation has not occurred.
Every race, rejection, activation, rollback and failed attempt is audited.

## Implementation status

The candidate implements immutable governance revisions, approval transitions,
maker-checker/quorum enforcement, optimistic activation, active/LKG pointers,
rollback, audit and outbox contracts. Evidence includes
`maker_checker_quorum_and_activation_are_enforced`,
`gateway_policy_http_enforces_maker_checker_replay_cas_tenant_and_lkg`, the
SQLite governance repository lifecycle suite, and the live PostgreSQL all-kind
lifecycle proof.
