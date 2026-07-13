# ADR 0003: Policy Approval, Activation, and LKG

- Status: Proposed
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

Snapshot and LKG foundations exist. Durable draft/revision/approval/activation
tables, quorum enforcement and transactional audit/outbox integration are
planned.
