# Audit, SIEM, and Evidence

## Status and Scope

This document defines the Phase 3 target for one durable, tamper-evident audit
contract across the Prodex data plane and control plane, asynchronous SIEM
delivery, evidence verification, retention, legal hold, and failure behavior.
It is a design and migration contract, not a claim that current local logs are
immutable audit evidence or that any deployment is certified.

Operational logs, metrics, traces, usage ledgers, and audit records remain
different products. A log message does not satisfy a mandatory audit write.
Remote SIEM delivery never executes synchronously in the request path and the
SIEM is never the system of record.

The terms **must**, **must not**, **required**, **should**, and **may** are
normative in this document.

## Current Repository Evidence and Gaps

| Area | Existing repository primitive | Gap against the target |
| --- | --- | --- |
| Local audit log | `prodex-audit-log` appends structured JSONL and supports bounded recent queries. `prodex audit` filters component, action, and outcome, with a 512-KiB read window. | Records contain arbitrary `details`, local process/time fields, and no tenant hash chain. This path is file-backed, not a durable multi-replica authority, and is not a complete query/export/verification interface. |
| Live call sites | Profile, broker, admin, and gateway paths emit local audit events. Several helpers intentionally discard append errors as best effort. | A discarded failure cannot satisfy mandatory audit. The data plane and control plane do not yet converge on one durable contract. |
| Typed audit domain | `prodex-domain` has bounded action/resource/reason types, tenant/principal identity, event IDs, outcomes, query/export/retention models, digest types, envelopes, and a canonical SHA-256 chain-link function. Debug output redacts sensitive fields. | The current event shape lacks all requested policy/classification/registry/provider/request/trace fields. Envelope link checking alone is not an end-to-end persisted-range verification or completeness proof. |
| Control-plane planning | Every current `ControlPlaneOperation` declares append-only hash-chain audit for success and denial; authorization plans carry typed events. | The plan is not proof that the live mutation and audit append commit atomically. Current policy routes do not yet implement the full governance lifecycle. |
| Storage plans | PostgreSQL and SQLite migrations include `prodex_audit_log`; plans enforce tenant matching, predecessor/event digests, unique tenant digests, query, export, and retention operations. PostgreSQL applies RLS. | The driver-free plans are not a unified production audit service. The schema has no chain sequence/checkpoint, governed revision fields, SIEM outbox/dead letter, legal-hold tables, or transactional mutation-plus-outbox proof. |
| Observability | `prodex-observability` has low-cardinality audit emit/persist/export, chain, query, and retention metric plans. | There are no SIEM delivery/backlog/dead-letter metrics or operational exporter wired to a durable outbox. |
| SIEM | None found in the production path. | A durable transactional outbox, exporter, sink contract, retry, deduplication, dead-letter operations, and lag alarms are required. |

The existing JSONL file remains useful as a personal-mode operational record
during migration. It must not be relabeled as the enterprise audit authority.

## Audit Invariants

1. One versioned audit contract represents data-plane and control-plane events.
2. Every material decision has a stable event identifier and a bounded,
   content-free payload.
3. A mandatory mutation, authorization, approval, activation, provider
   dispatch, or other high-risk action is not acknowledged until its required
   durable audit record has committed according to the failure matrix.
4. Audit records are append-only. Corrections, retention actions, export, and
   verification results create new events; they do not edit history.
5. Per-tenant or explicitly defined tenant-partition chains are tamper-evident,
   concurrency-safe, and independently verifiable.
6. The same authoritative database transaction writes a material control-plane
   mutation, its audit event, and its SIEM outbox item when they must agree.
7. Provider, policy, registry, approval, session, accounting, and audit
   identifiers are references, never raw content.
8. Prompt, response, tool arguments/results, detector matches, PII, secrets,
   tokens, credentials, full IP addresses, arbitrary headers, filenames, and
   arbitrary policy text are prohibited in audit and outbox payloads.
9. Query, export, retention, and delivery collections, pages, batches, retries,
   leases, diagnostic strings, and metric labels are bounded.
10. PostgreSQL is authoritative in enterprise multi-replica modes. Redis and
    SIEM contain only rebuildable delivery/coordination copies.

## Versioned Audit Event

The target event is an immutable typed value. Its canonical serialization is
versioned and independent of map iteration order or locale.

~~~text
GovernanceAuditEvent
  schema_version
  event_id
  occurred_at
  recorded_at
  tenant_id
  partition_id and sequence
  pseudonymous principal reference and principal kind
  action and resource kind/reference
  outcome and stable bounded reason codes
  request_id and trace_id when applicable
  session reference when applicable
  policy, classification, detector, registry, routing, pricing revisions
  classification and inspection coverage
  provider instance/revision/deployment/model references when applicable
  approval, break-glass, reservation, and idempotency references when applicable
  bounded non-content flags and numeric summaries
  previous digest
  event digest
~~~

Tenant and principal references are opaque identifiers or a versioned keyed
pseudonym appropriate to the authorized query model. Keyed pseudonyms need a
rotation and verification design; hashing a guessable email without a key is
not anonymization. Raw source identity claims remain in the identity system,
not copied into audit.

Stable actions cover at least:

- authentication, authorization, denial, rate/admission, and session
  revocation outcomes;
- inspection/classification coverage, transformations, and guardrail outcomes;
- PDP decisions and obligation enforcement;
- policy draft, validation, submission, approval, rejection, activation,
  rollback, invalidation, and LKG promotion;
- provider registry changes, routing selection, fallback, revocation, and
  dispatch authorization;
- execution approval and break-glass grant, use, revocation, and review;
- reservation, reconciliation, and failure recovery links;
- audit query, export, verification, retention, hold, purge, and dead-letter
  operations; and
- failed administrative attempts, cross-tenant denials, and stale/replayed
  mutation attempts.

An event records categories and counts, not matched values. For example,
inspection audit may contain `finding_category=credential` and a capped count;
it must not contain the credential, match span, source filename, or field text.

## Chain, Append, and Completeness

### Chain construction

Each tenant uses a bounded partition strategy, such as tenant plus an approved
time epoch. A partition has a durable head containing the next sequence and
last digest. Append locks or atomically compares that head, assigns exactly one
sequence, computes the versioned digest over canonical event bytes plus the
partition, sequence, and predecessor, inserts the event, then advances the head
in one transaction.

The current `compute_audit_chain_digest` is a useful version-1 primitive. The
target must extend or version it rather than silently changing its digest
domain. Verification recomputes every digest, checks tenant/partition/sequence,
checks every predecessor, confirms declared start and end anchors, and reports
gaps, duplicates, invalid canonical bytes, and unexpected chain forks.

Partitioning avoids one global audit lock. Partition rollover writes linked
close/open checkpoint events. It must not allow an attacker to hide deletion at
the boundary. Cardinality is bounded and a tenant cannot choose arbitrary
partition identifiers.

### Checkpoints and anchors

Periodic checkpoints include partition, sequence range, first/last digest,
event count, schema versions, creation time, and checkpoint digest. Where an
external signing or anchoring service is configured, it receives only these
digests and metadata. Its secret key is referenced through `SecretRef`, never
stored in the audit database or outbox payload.

An isolated restore must verify from a trusted retained anchor, not solely from
mutable pointers restored with the same database. An anchor proves consistency
with a recorded checkpoint; it does not by itself prove that every expected
business event was emitted. Completeness also requires transactional emission,
reconciliation, expected-count checks, and application-level evidence.

### Material operation transaction

For an authoritative control-plane mutation, one database transaction:

1. authenticates tenant-scoped storage context and enforces optimistic
   preconditions;
2. writes the immutable business revision or mutation;
3. appends the typed audit event and advances the chain head;
4. inserts the SIEM outbox item using the same event ID; and
5. records idempotency completion before committing.

All five commit or none. An unknown commit outcome is resolved through the
tenant, operation, idempotency key, and request fingerprint before retry.

For a high-risk data-plane dispatch, `bank_enforce` first durably appends the
authorized decision/dispatch-intent event before invoking the provider. The
event links the reservation and immutable decision revisions. Completion,
denial, termination, and reconciliation append follow-up events. This avoids
dispatching an unaudited request while preserving the rule that an already
committed upstream stream cannot be rotated or rewritten because a later audit
write fails.

## SIEM Transactional Outbox

The SIEM exporter is a separate bounded background worker. It never receives
raw request/model content and never blocks a request on remote network I/O.

The minimum outbox record contains:

~~~text
delivery_id (normally the stable audit event ID plus sink ID)
tenant_id
audit_event_id and event digest
sink configuration revision reference
bounded serialized event envelope or immutable event reference
state: pending | leased | delivered | retry_wait | dead_letter
attempt count, next attempt time, lease owner/expiry
created time, delivered time
last failure category and bounded redacted diagnostic code
version/ETag
~~~

The sink configuration contains a validated endpoint reference, TLS identity
and trust references, format/version, batch limit, timeout, and `SecretRef`.
Credential material is resolved by the exporter after a record is leased. URLs
with embedded credentials, arbitrary per-event endpoints, and redirects to
unapproved hosts are rejected.

### Delivery behavior

- Delivery is at least once. The stable delivery/event ID is sent as the sink
  idempotency key when supported.
- Workers claim bounded batches with a lease and concurrency cap. PostgreSQL
  may use a tenant-safe `FOR UPDATE SKIP LOCKED` pattern.
- Success marks delivery only after the sink acknowledges the configured
  contract. Partial batch success is recorded per item.
- Retry uses bounded exponential backoff, maximum delay, maximum attempts, and
  a configured maximum age. Authentication/configuration and permanent schema
  failures may dead-letter immediately.
- A crashed worker's expired lease is reclaimable. Duplicate delivery is safe
  and observable.
- Exhausted records move transactionally to a dead-letter table or terminal
  state with an append-only audit event. They are never silently dropped.
- Requeue, discard after approved retention, sink change, and manual resolution
  require control-plane authorization, idempotency, optimistic preconditions,
  mandatory audit, and maker-checker when policy marks the operation high risk.

Remote sink outage increases backlog and lag; it does not roll back an already
durably audited operation. Backpressure is bounded by database quotas and
operator thresholds. At hard storage limits, the mandatory-audit failure matrix
applies to new high-risk operations rather than dropping old events.

## Query, Export, Verification, Retention, and Legal Hold

### Query and export

Queries are tenant-scoped and authorized before storage access. They have a
bounded time range, cursor, page size, sort order, action/outcome/reason filters,
and allowed projection. Default APIs do not permit free-text searches across
arbitrary payload values.

Exports are asynchronous for large ranges. An export manifest records query
fingerprint, tenant, range, event count, schema versions, first/last sequence
and digest, checkpoint/anchor references, artifact digest, format, expiry, and
the audit event authorizing export. Export artifacts are encrypted, access
controlled, time limited, and never placed in a public URL. CSV output defends
against spreadsheet formula injection; JSON/JSONL uses a versioned schema.

Verification can check one link, a bounded range, a complete partition, an
export manifest, or a restored database. It returns stable categories such as
`verified`, `gap_detected`, `digest_mismatch`, `unexpected_fork`,
`anchor_unavailable`, and `range_incomplete`; it does not print sensitive event
fields to prove a failure.

### Retention and legal hold

Retention policy is tenant- and classification-aware, revisioned, approved,
and audited. Purge selects a bounded batch, rechecks policy and hold membership
transactionally, writes a purge manifest/checkpoint and mandatory audit event,
then deletes only eligible partitions or rows according to the approved chain
retention design. A missing or ambiguous hold decision prevents deletion.

Legal holds are durable, scoped, versioned resources with maker-checker
creation and release where required. The hold stores identifiers and criteria,
not copied prompt/response content. Purging an event covered by an active hold,
rewriting a chain to hide purge, or deleting outbox evidence before its policy
allows is prohibited.

## Failure Matrix

| Failure or operation | Required behavior |
| --- | --- |
| Mandatory control-plane audit append, chain-head update, or outbox insert fails before commit | Roll back the business mutation and idempotency completion. Return a stable service-unavailable error. In `bank_enforce`, alert and never acknowledge success. |
| Mandatory high-risk data-plane dispatch-intent append fails before provider dispatch | Do not dispatch. Release or safely expire any reservation. Return a stable local precommit denial/service-unavailable result according to the route contract. |
| Follow-up outcome append fails before the first local response commit | Do not commit the local response in `bank_enforce`; terminate or reject using the existing stable local precommit boundary and reconcile usage. Never synthesize an upstream quota error. |
| Follow-up outcome append fails after a stream is committed | Do not retry or rotate the provider and do not rewrite prior chunks. Terminate safely if policy requires, mark the pre-existing durable intent for reconciliation, stop admitting new mandatory-audit work for the affected scope, degrade readiness, and alert. |
| SIEM sink is unavailable, slow, or returns a transient error | Keep the authoritative audit and outbox records, retry asynchronously with bounded backoff, expose lag, and continue data-plane service while durable capacity remains. |
| SIEM payload is permanently rejected or retry budget is exhausted | Move to dead letter, append a durable delivery-failure audit event, alert, and require an authorized resolution. Do not drop or block on remote delivery. |
| Outbox backlog reaches warning threshold | Alert and scale/fix exporter; no request-path remote calls. |
| Outbox/audit storage reaches hard safety limit | Apply admission controls. In `bank_enforce`, fail new operations requiring mandatory audit closed instead of dropping records or evicting unexpired evidence. |
| Chain predecessor conflict or unexpected fork occurs | Roll back the append, quarantine the partition, retry only after bounded head refresh, and fail mandatory operations for that partition if integrity cannot be restored. Alert immediately in `bank_enforce`. |
| Audit database is unavailable during a read/query/export | Return a stable service-unavailable result for that operation. Do not weaken data-plane policy because an audit read failed. |
| Audit database is unavailable during a mandatory append | Apply the relevant precommit or postcommit rule above. A local JSONL fallback is not authoritative in enforcement modes. |
| Verification or anchor check fails | Mark evidence unverified, prohibit destructive purge and claims of integrity, preserve artifacts, and open an incident. Existing provider streams are not retroactively changed. |
| Legal-hold lookup is missing, stale, or inconsistent during purge | Fail the purge closed and audit the failed attempt when the chain is available. |
| Retention worker crashes or a purge commit is ambiguous | Resolve by purge operation/idempotency key and manifest before retry. Never rerun an unbounded delete. |
| Redis is unavailable | Outbox, audit authority, legal holds, and retention remain in PostgreSQL. Rebuild optional coordination; do not lose or acknowledge records from Redis. |
| Personal-mode local JSONL append fails | Report or log the bounded local diagnostic according to current compatibility behavior. Do not present the record as durable enterprise evidence. |

In `enterprise_enforce`, any operation whose active policy marks audit mandatory
uses the same fail-closed rules as `bank_enforce`. `bank_enforce` makes mandatory
audit non-optional for identity, policy, approval, provider selection/dispatch,
break glass, session revocation, and other defined high-risk actions.

## API, CLI, and Control-Plane Surface

The checked OpenAPI target includes authorized, tenant-scoped capabilities for:

- paginated audit query and event metadata retrieval;
- start/status/download of a bounded export;
- verify link, range, partition, export manifest, and checkpoint;
- inspect chain/checkpoint and SIEM outbox health without content leakage;
- list and inspect dead letters, then requeue or resolve them;
- create/list/release legal holds;
- create/inspect retention plans and execute bounded purge; and
- inspect evidence manifests and restore-verification results.

Mutation endpoints require operation-specific control-plane scope,
`Idempotency-Key`, `If-Match`, tenant/resource authorization, mandatory audit,
and maker-checker where configured. Query/export permissions remain separate
from retention, hold, dead-letter, and sink-administration permissions. A
tenant administrator cannot query another tenant or infer it from errors,
cursors, counts, unique conflicts, or delivery status.

The CLI should provide equivalent versioned JSON and human output, for example:

~~~text
prodex audit query ...
prodex audit verify ...
prodex audit export create|status ...
prodex audit hold create|list|release ...
prodex audit retention plan|purge ...
prodex audit siem status|dead-letter|requeue ...
~~~

The existing `prodex audit` JSONL reader remains a clearly labeled local
compatibility command during migration. It must not silently switch query
semantics or imply chain verification. Any compatibility alias has a removal
condition, test, and operator notice outside the Codex TUI runtime.

## Metrics, Alerts, and Runbook Inputs

Metrics use bounded labels such as operation, result, mode, sink class,
failure category, and coarse backlog bucket. Tenant, principal, request,
event, policy, provider, URL, filename, and reason text are not metric labels.

Required signals include append latency/result, chain conflicts, verification
result, audit-to-mutation rollback count, outbox depth/oldest age/claim rate,
delivery latency/result, retry count, dead-letter count, lease expiry, export
age/result, purge result, legal-hold conflicts, checkpoint/anchor age, and
reconciliation lag. Alerts cover mandatory-append failure, chain fork/gap,
hard-capacity risk, dead letters, sustained exporter lag, stale anchors,
unexpected purge, and verification failure.

Runbooks must identify safe actions for exporter outage, credential expiry,
sink schema rejection, partition quarantine, database failover, outbox growth,
unknown commits, dead-letter requeue, legal-hold conflict, restore verification,
and evidence preservation. A runbook cannot authorize deleting evidence or
disabling mandatory bank audit through an ordinary feature flag.

## Storage Migrations

Versioned PostgreSQL and SQLite migrations add, at minimum:

- versioned audit-event payload columns or a new typed event table;
- chain partition heads, sequences, checkpoints, and anchor references;
- SIEM sink references, outbox, delivery attempts, leases, and dead letters;
- export jobs and manifests;
- retention revisions, purge operations/manifests, legal holds, and membership;
- idempotency links and governed revision foreign keys; and
- indexes for bounded tenant/time/sequence/status queries.

PostgreSQL tables use tenant-aware keys, RLS with transaction-local tenant
context, least-privilege writer/exporter/retention roles, and constraints that
prevent cross-tenant references. Audit writers cannot update or delete event
rows. Exporters can claim outbox rows but cannot mutate business resources or
audit payloads. Retention workers can delete only through approved bounded
plans and cannot release holds.

Migrations are external, checksummed, expand/backfill/validate/cutover/contract
steps. They never run in a gateway request path. Backfill imports legacy JSONL
as explicitly marked legacy evidence with source-file and import-manifest
digests; it must not pretend that old events had an original hash chain or
tenant identity that was never recorded. Malformed or content-leaking legacy
records are quarantined through a redacted manifest rather than copied into the
new authority.

SQLite supports personal/local compatibility and deterministic adapter tests;
it is not a substitute for PostgreSQL HA or RLS in enterprise bank deployment.

## Migration and Cutover

1. Add typed event/schema extensions, transactional append, chain verification,
   and outbox storage while the JSONL path remains the documented legacy
   authority for personal mode.
2. Shadow-write sanitized typed events and compare event coverage, latency,
   failure behavior, and leakage without treating two logs as authoritative.
3. For canary tenants, make the durable typed append authoritative before
   high-risk mutations and dispatch. Keep JSONL only as an operational mirror.
4. Enable the exporter against a synthetic sink, then a tenant-approved sink.
   Exercise outage, duplicate delivery, credential rotation, dead letter,
   backlog, and recovery.
5. Enable chain checkpoints, anchors, export verification, retention, and
   legal-hold operations before any governed purge.
6. Expand enforcement after restore and multi-replica evidence passes. Rollback
   disables new target use but preserves all committed target events/outbox
   rows; it never rewrites the chain.
7. Remove best-effort security-audit call sites after every data/control-plane
   path uses the typed service. Retain a separately named operational log only
   where it still serves a non-audit purpose.

There is exactly one audit authority at each cutover stage. Dual emission is a
comparison mechanism, not permission to accept success when the authoritative
append fails.

## Verification and Evidence Gate

Required evidence includes:

- canonical serialization, digest, predecessor, sequence, partition rollover,
  fork, gap, duplicate, tamper, and anchor verification tests;
- property/fuzz tests for audit parsing and serialization without content or
  allocation blowups;
- transaction tests proving mutation, audit, outbox, and idempotency commit all
  or none, including unknown commit outcomes and concurrent appends;
- PostgreSQL RLS and least-privilege role tests plus SQLite compatibility and
  forward/backward migration tests;
- data-plane precommit/postcommit mandatory-audit failure tests for unary, SSE,
  and WebSocket flows without retry after commit;
- SIEM retry, bounded backoff, deduplication, partial batch, expired lease,
  dead-letter, requeue, credential rotation, sink SSRF, TLS, and outage tests;
- query pagination, tenant isolation, export manifest, encryption/access,
  formula-injection, retention, hold, and bounded purge tests;
- no-content-in-audit/outbox/log/metric/trace/error snapshots;
- backup/isolated restore verification of events, heads, checkpoints, anchors,
  outbox, dead letters, holds, and policy/registry links; and
- load tests for append contention, audit pressure, exporter outage, maximum
  backlog, and recovery without unbounded queue, CPU, memory, or storage growth.

Phase 3 audit work exits only when every material decision uses the one typed
durable contract, mandatory failure behavior is enforced, SIEM delivery is
asynchronous and recoverable, hash-chain/export/restore verification is proven,
and no live security path depends on discarded JSONL append failures.
