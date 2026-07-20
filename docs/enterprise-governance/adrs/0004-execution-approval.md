# ADR 0004: High-Risk Execution Approval

- Status: Accepted
- Scope: enabled only for policy effects requiring approval

## Context

Approving a policy revision does not authorize every high-risk request made
under it. A generic approval flag is replayable and can drift from the request
that was reviewed.

## Decision

Represent `require_approval` as a policy effect. Create a content-free approval
request bound to decision ID, input fingerprint, tenant, principal/operation,
classification, obligation summary, requested provider class, policy revision,
expiry and maximum uses. Independent approvers issue a revocable token bound to
those fields. Validate and consume it atomically with idempotency/reservation
before dispatch. Any relevant request mutation or revision mismatch requires a
new decision and approval. Approval cannot expand the active policy's eligible
set or bypass a mandatory obligation.

## Consequences

Approval state needs durable uniqueness, expiry, revocation and use accounting.
Retries with the same idempotency key observe one outcome. Raw request content
is not stored in approval records.

## Implementation status

The optional execution-approval effect is enforced in the gateway before
provider dispatch. The production PEP derives a content-free fingerprint from
the tenant, principal, governance session, action, model, requested tools,
request-body digest and active policy revision. The governance repository owns
quorum approval, expiry, revocation and atomic single-use consumption together
with create/consume audit-outbox writes. Tenant-scoped administration routes
and regression tests cover exact binding, quorum, replay and one-use behavior.
