# Approval and Break-Glass

The target has two separate workflows: approval of policy revisions and
approval of a particular high-risk execution. Break-glass is a third, narrower
emergency grant. They must not share a broad bypass flag. The current tranche
implements durable maker-checker, policy-selected execution approval, and a
bounded break-glass approval/activate/revoke path. Retention purge accepts only
an independently approved active grant with exact tenant and audit-retention
scope, a maximum one-hour lifetime, bounded purge batches, and mandatory audit.
The fuller persistence design is in
[`07-policy-approval-and-store.md`](07-policy-approval-and-store.md).

## Policy revision workflow

```text
draft -> submitted -> approved -> active -> superseded
                   \-> rejected
active -> invalidated
active -> prior immutable approved revision (rollback activation)
```

- Drafts are mutable and never authoritative.
- Submission freezes a candidate checksum and validation result.
- The maker cannot satisfy the checker quorum for the same revision.
- Approval binds reviewers, roles, tenant, checksum, policy kind and expiry.
- Activation is transactional with active/LKG pointers, mandatory audit and
  invalidation/outbox records.
- Rollback activates a prior immutable approved revision; it does not mutate
  history or resurrect a revoked revision.

High-impact changes require the configured independent quorum. Self-approval,
replayed approval, stale checksum, expired approval and conflicting concurrent
activation all fail closed.

## Execution approval

A `require_approval` policy decision creates a content-free request containing
the decision ID, input fingerprint, tenant, operation, classification,
obligation summary, requested provider class, expiry and maximum uses. The
approved token is bound to all of those fields. Any request mutation requires a
new decision and approval.

Approval is consumed atomically with idempotency and reservation. An expired,
revoked, used-up or mismatched approval is denied. Approval never permits a
provider, region, tool or retention choice excluded by the active policy.

Current evidence binds a SHA-256 request-body digest with tenant, principal,
session hash, operation, model, tools and policy revision; enforces configured
quorum and one use; and lists, shows and reviews approvals over content-free
administrative HTTP representations. Raw request payloads are not stored or
returned.

## Break-glass

Break-glass is disabled by default and cannot override these invariants:

- tenant isolation, authentication or target tenant binding;
- cryptographic audit integrity or evidence retention;
- prohibited provider/region/data residency rules;
- secret exfiltration protections;
- revoked credentials, policy or registry entries; or
- transport affinity after a response is committed.

A grant names the exact tenant, principal, operation, reason, incident/ticket,
scope, expiry, use limit and independent approver. It is short-lived, revocable,
single-purpose and reviewed after use. Creation, approval, use, denial,
revocation, expiry and review are mandatory content-free audit events.

## Failure behavior

| Failure | Required result |
| --- | --- |
| Approval database/audit transaction unavailable | Deny; never acknowledge approval or activation |
| Checker is maker or lacks current role | Deny with stable reason code |
| Token checksum/input mismatch | Deny and audit replay/mismatch signal |
| Break-glass expiry/use counter uncertain | Deny |
| Outbox unavailable for mandatory mutation | Roll back the business mutation |

Observe mode may shadow the execution decision but does not make a false
approval. `enterprise_enforce` and `bank_enforce` enforce the workflow;
`bank_enforce` also rejects insecure startup configuration.

## Acceptance evidence

- state-machine, concurrency and transaction tests;
- maker-checker separation and quorum property tests;
- stale, replayed, self-approved, revoked and exhausted execution-token tests;
- break-glass independent-approval, scope, expiry and revocation tests; and
- audit/outbox failure tests proving no successful mutation escapes evidence.
