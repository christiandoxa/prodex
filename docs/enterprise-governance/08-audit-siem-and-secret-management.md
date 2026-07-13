# Audit, SIEM, and Secret Management

The target couples mandatory durable audit to governed mutations and selected
high-risk dispatches, then exports sanitized evidence asynchronously through a
transactional outbox. Secrets are references resolved from an approved secret
manager, never policy values. The repository already has append-only audit,
redaction, secret-store and hash-chain primitives; transactional governance
audit/outbox, SIEM delivery and external Vault integration remain gaps. Detailed
design is in [`08-audit-siem-and-evidence.md`](08-audit-siem-and-evidence.md).

## Audit event

Each versioned event contains a stable event ID, timestamp, tenant partition,
actor and session references, operation/resource, outcome and reason codes,
request/decision/policy/registry/provider revisions, classification and
inspection coverage, approval/break-glass/idempotency references when relevant,
previous-event hash and event hash. It never contains prompt/response bodies,
raw findings, bearer tokens, cookies, provider credentials, raw session IDs or
unbounded exception text.

Hash chaining detects alteration; it does not replace database authorization,
encryption, retention, backup or independent export. Verification runs during
startup sampling, scheduled jobs, export and incident response.

## Mandatory transaction boundary

Policy activation, registry activation, approval state changes, break-glass
changes, secret-reference changes and other control-plane mutations commit with
their audit and outbox records in one database transaction. For a policy-marked
high-risk dispatch, the required pre-dispatch evidence is durable before the
upstream commitment. If mandatory audit cannot commit, the operation fails
closed instead of succeeding invisibly.

## SIEM outbox

Workers claim bounded batches with leases, publish idempotently by event ID,
record attempts, back off with jitter and move poison records to a visible
dead-letter state. Backlog age/depth and delivery failures alert operators. SIEM
is never called synchronously from the request hot path. Backpressure applies
admission or fails mandatory operations closed before storage exhaustion; it
never drops unexpired evidence.

## Secrets

- Configuration and policy store opaque `secret_ref` values only.
- An allow-list maps a tenant/purpose to permitted secret namespaces.
- Resolution occurs in a bounded background cache or launch boundary, not in a
  per-chunk streaming path.
- Values are held for the minimum lifetime, excluded from Debug/log/audit/error
  output, and rotated without embedding a new value in policy.
- Cache expiry, revocation and Vault outage behavior is explicit per mode.
- `bank_enforce` rejects raw secret configuration and missing external secret
  prerequisites at startup.

## Failure matrix

| Failure | Result |
| --- | --- |
| Mandatory audit/hash-head/outbox insert fails | Roll back mutation or deny precommit dispatch |
| SIEM unavailable | Retain/retry outbox, alert on age; never block hot path directly |
| Chain fork/integrity failure | Quarantine partition, alert, fail mandatory writes until resolved |
| Secret unavailable/expired | Deny operations requiring it; never reuse beyond declared safe lease |
| Redaction/schema failure | Emit bounded fallback event or fail mandatory operation; never log raw input |

## Acceptance evidence

- transaction rollback and concurrent chain-head tests;
- tamper, fork and deterministic verifier tests;
- duplicate delivery, retry, lease expiry, dead-letter and backlog chaos tests;
- secret redaction/canary scans and rotation/revocation/Vault outage tests; and
- proof that audit and diagnostic schemas remain content-free.
