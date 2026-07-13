# Storage, High Availability, Backup, and Disaster Recovery

## Status and Scope

This document separates repository evidence observed on 2026-07-13 from the
storage and recovery target required by the enterprise-governance design. It is
an implementation plan and operational contract, not evidence of production
readiness, certification, or regulatory compliance.

The scope includes authoritative governance and accounting state, tenant
isolation, migrations, cache and coordination state, service availability,
backup encryption, restore verification, recovery objectives, and failure
drills. Secret-manager backup and recovery remain the responsibility of the
configured secret-management system; database backups must contain secret
references rather than plaintext secret material.

The terms in this document have the following meaning:

- **Existing evidence** is behavior or configuration present in the current
  repository.
- **Target** is required behavior that is not proven merely by this document.
- **Gap** is work or operational evidence still needed before the target can be
  treated as implemented.

## Existing Evidence and Material Gaps

| Area | Existing evidence | Gap against the target |
| --- | --- | --- |
| PostgreSQL schema | External migrations cover tenant/accounting plus four-kind governance revisions/pointers/approvals, sessions/revocations, audit-chain metadata, SIEM outbox and dead letters. | Optional execution approvals and legal-hold workflow remain disabled capabilities. |
| Tenant isolation | The initial migration enables RLS and creates tenant policies using `current_setting('prodex.tenant_id', true)`. The runtime sets that value within a transaction. The PostgreSQL proof and backup drill include cross-tenant checks using a `NOSUPERUSER NOBYPASSRLS` role. | The migrations do not use `FORCE ROW LEVEL SECURITY`; table-owner bypass, complete role/grant separation, connection-pool reset behavior, and isolation of all future governance tables are not yet proven. |
| Migration boundary | `prodex-storage-postgres` rejects DDL in gateway startup and request-path modes. The Kubernetes manifest supplies a distinct migration Job and service account. Governance migrations are idempotency-tested. | Managed-environment downgrade/forward-recovery rehearsal remains operator evidence. |
| SQLite | `prodex-storage-sqlite-runtime` implements transactional four-kind lifecycle, session, audit/export and outbox compatibility operations over the driver-free SQLite plan crate. | SQLite does not provide the multi-replica HA or PostgreSQL RLS boundary required by enterprise enforcement. |
| Redis | `prodex-storage-redis` has a core contract that rejects authoritative durable state and provides bounded coordination and rate-limit behavior. | Compatibility configuration and documentation still permit Redis-backed gateway state. That ambiguity must be removed for enterprise enforcement modes. Redis loss and failover behavior are not proven in an environment drill. |
| Gateway availability | The Kubernetes baseline declares three gateway replicas, topology spreading, a PodDisruptionBudget with `minAvailable: 2`, an HPA, readiness/liveness/startup probes, and graceful termination. | The manifest does not deploy PostgreSQL HA. The control plane has one replica and no equivalent disruption or topology policy. End-to-end failover under active unary and streaming traffic is not proven. |
| Backup and restore | `scripts/ci/backup-restore-drill.mjs` creates a PostgreSQL custom dump, restores it, compares tenant/accounting/governance fingerprints, checks a recovery-point marker, validates policy/provider/session/audit/SIEM links, and tests RLS. It writes redacted JSON evidence. | Legal holds, encryption-key separation, WAL/PITR, regional recovery and application cutover remain environment responsibilities. |
| Recovery thresholds | The synthetic drill has configurable limits and defaults of 60 seconds for its `rpo_seconds` value and 300 seconds for restore duration. | Those CI thresholds are not approved production RPO/RTO objectives. The current `rpo_seconds` calculation measures elapsed time from dump recovery point to restored verification, not the amount of acknowledged production data lost at a declared disaster cutoff. |
| Audit durability | Governed data-plane decisions use a bounded writer with commit acknowledgement; SQLite/PostgreSQL atomically append digest-linked audit plus SIEM outbox records. Restore verifies governance audit links. | Legacy personal-mode JSONL remains operational logging, not enterprise authority. Independent external chain anchors remain deployment evidence. |
| Drill execution | The disposable PostgreSQL/TLS/RLS proof and custom dump/restore drill passed. Measured synthetic RPO was 1.65 seconds and restore RTO was 1.243 seconds. | These synthetic values are not approved production RPO/RTO objectives. |

## Authoritative Storage Contract

### Target ownership by backend

| Data class | Authoritative target | Non-authoritative copies |
| --- | --- | --- |
| Tenant, user, service identity, role, scope, and credential metadata | PostgreSQL | Bounded in-process caches keyed by tenant and revision |
| Access sessions, revocations, and classification/policy pins | PostgreSQL | Expiring caches that can be rebuilt and invalidated |
| Immutable policy revisions, approval records, activation history, active and last-known-good pointers | PostgreSQL | Revision-addressed evaluator cache |
| Classification revisions and rules | PostgreSQL | Revision-addressed classifier cache |
| Provider registry, routing rules, capability declarations, and pricing revisions | PostgreSQL | Revision-addressed routing cache |
| Budgets, reservations, usage events, billing ledger, and idempotency records | PostgreSQL | Redis coordination hints and bounded local read caches |
| Audit chain, retention state, legal holds, SIEM outbox, and SIEM dead letters | PostgreSQL | SIEM is a downstream export, not the system of record |
| Rate-limit windows, short-lived locks, cache entries, and rebuildable coordination | Redis | None required for durable recovery |
| Raw provider and identity secrets | External secret manager through `SecretRef` | Short-lived zeroized process memory only |

For multi-replica enterprise deployment, PostgreSQL is the sole authoritative
application database. SQLite and file-backed state are limited to personal,
development, migration tooling, or explicitly documented single-node recovery
work. They must be rejected at startup when an enforcement mode requires the
enterprise durability boundary.

Redis must never be the authority for identity, session revocation, policy,
approval, classification, provider registry, pricing, credential references,
budgets, reservations, usage ledger, audit, SIEM delivery, retention, or legal
hold state. Deleting every Redis key must not delete an acknowledged business
record or alter the active governance revision. Redis recovery should rebuild
from PostgreSQL and current traffic rather than restore Redis as a source of
truth.

The bank-enforcement target must fail closed when authoritative state needed to
authorize, classify, reserve, audit, or execute a request cannot be read or
committed. Exact behavior for an unavailable Redis coordination service must be
defined per operation: security and budget limits must become conservative,
while no code may silently substitute Redis data for PostgreSQL authority.
These deployment modes and startup gates are target work; they are not present
under those names in the current repository.

## Target Governance Schema

The PostgreSQL model should add versioned migrations for at least the following
bounded, tenant-scoped entities. Names are conceptual until the migration and
domain-model reviews approve concrete table names.

### Identity and session state

- service-identity credential bindings, allowed scopes, certificate or workload
  identity bindings, status, and rotation metadata;
- access sessions with principal, tenant, authentication method, issued time,
  absolute expiry, idle expiry, last activity, credential scope, policy
  revision, classification, device/network context digest, and status;
- session and credential revocations with reason, actor, effective time, and
  audit-event link;
- concurrency leases or counters only where they can be updated atomically and
  recovered without Redis authority.

### Policy, classification, and approval state

- immutable policy revisions containing canonical source, compiled artifact
  digest, author, state, creation time, and validation result;
- tenant active-policy and last-known-good pointers with monotonic revision and
  atomic activation preconditions;
- policy submissions, maker-checker approvals, rejection, expiry, quorum, and
  activation or rollback history;
- immutable classification-rule revisions plus active and last-known-good
  pointers;
- execution approvals with request fingerprint, obligations, approvers,
  expiry, consumption state, and exactly-once execution key.

Published revisions must not be mutated in place. Activation must atomically
link the approved revision, activation event, audit event, and cache
invalidation/outbox record. A rollback activates a prior immutable revision; it
does not rewrite history.

### Provider and accounting state

- tenant provider registry entries, enabled status, region and data-processing
  attributes, capability declarations, and `SecretRef` credential binding;
- immutable routing and pricing revisions, active pointers, health-policy
  metadata, and deterministic selection inputs;
- existing budget policy, reservation, usage ledger, and idempotency entities,
  extended only through online-compatible migrations and explicit invariants;
- referential links from each governed invocation to policy, classification,
  registry/routing, pricing, request, trace, session, principal, and approval
  identifiers as applicable.

### Audit, SIEM, retention, and recovery state

- an append-only audit chain with canonical payload digest, previous digest,
  sequence or other concurrency-safe predecessor, tenant, principal, action,
  outcome, reason, governed revision identifiers, request ID, and trace ID;
- chain anchors or checkpoints whose storage and signing-key lifecycle permit
  an isolated restore to prove completeness without trusting restored mutable
  pointers alone;
- a transactional SIEM outbox written in the same database transaction as the
  audited mutation, including stable delivery ID, attempt schedule, and
  idempotency key;
- SIEM dead letters with bounded diagnostic metadata, retry history, operator
  disposition, and audit links;
- retention policies, purge execution records, legal holds, hold membership,
  and release approvals that prevent covered records from being deleted;
- backup catalog and restore-drill evidence references containing artifact and
  manifest digests, but no database credentials, plaintext tenant data, or
  encryption keys.

Every tenant-owned table must carry an immutable tenant identifier, tenant-aware
primary or unique keys, and tenant-scoped foreign keys where PostgreSQL permits
them. Globally managed reference data needs an explicit ownership and
authorization design rather than an omitted tenant key by accident. Raw prompt,
response, token, credential, or secret content must not be added merely to make
recovery verification convenient.

## PostgreSQL Isolation and Role Target

RLS is defense in depth, not a replacement for tenant-aware domain types and
query parameters. The target database contract is:

1. Enable and **force** RLS on every tenant-owned table.
2. Run gateways and control-plane services as `NOSUPERUSER NOBYPASSRLS` roles
   that do not own tenant tables and cannot alter schemas or policies.
3. Use a separate, narrowly scoped migration identity. The migration identity
   is unavailable to request-serving pods.
4. Set the tenant context transaction-locally before every tenant query. Abort
   when it is absent, malformed, or inconsistent with the authenticated
   request context.
5. Reset or discard pooled connections after transaction errors so tenant
   context cannot leak between borrowers.
6. Apply both `USING` and `WITH CHECK` policies and test read, insert, update,
   delete, upsert, prepared statement, function, trigger, and foreign-key paths.
7. Deny direct table privileges by default. Grant only the required operations,
   and keep audit mutation, retention, and migration privileges separate.
8. Prove owner, privileged maintenance, application, exporter, backup, and
   restore roles independently. A passing test with one non-owner role is not
   sufficient evidence for the full role design.

Future migration tests must enumerate the schema and fail when a tenant-owned
table lacks forced RLS, matching policies, or tenant-aware keys. Tests must also
demonstrate that one tenant cannot infer another tenant through counts, unique
constraint errors, idempotency replay, outbox delivery, or restore tooling.

## Migration Rules

Only the external migrator may execute DDL. Gateway startup may compare the
observed schema version with its supported compatibility window and fail before
listening; request, stream, background export, and cache-refresh paths must
never create or alter tables.

Target migration requirements are:

- immutable, checksummed, monotonically ordered migration artifacts;
- a migration-history table recording version, checksum, phase, actor/build,
  start and completion times, and outcome;
- one database-wide migration lock or equivalent serialization mechanism;
- transactional DDL where PostgreSQL supports it and an explicit recovery plan
  where it does not;
- expand, bounded backfill, dual-read/write only when justified, validation,
  cutover, and contract phases for incompatible changes;
- backfills that are resumable, rate limited, observable, tenant-safe, and do
  not hold unbounded locks;
- compatibility across the versions that can coexist during a rolling update;
- a tested forward-fix or rollback procedure before production execution;
- preflight capacity, lock, replica-lag, backup recency, and restore-readiness
  checks for destructive or high-volume work;
- no deletion of audit, legal-hold, approval, or ledger history through a
  generic schema rollback.

The existing phase enum and DDL boundary are useful evidence, but the current
catalog contains only two expand migrations. They do not prove this lifecycle.

## High-Availability Target

### Application services

- Run at least two healthy instances of each required data-plane, control-plane,
  policy, inspection, audit-export, and migration-coordination service across
  independent failure domains, except one-shot Jobs.
- Use readiness that reflects the ability to serve the route safely without
  turning temporary downstream degradation into an unsafe healthy state.
- Drain unary and streaming work during rollout without mid-stream provider
  rotation or duplicate accounting commits.
- Make retries idempotent and bounded. Database failover must not turn an
  unknown commit outcome into duplicate approval consumption, ledger events,
  audit events, or provider invocation.

The checked-in gateway Deployment is a useful three-replica scaffold. The
single-replica control-plane Deployment and missing exporter/worker workloads
remain gaps.

### PostgreSQL

The deployment target is a supported PostgreSQL topology with automated
failure detection, a primary plus replicas in separate failure domains,
fencing against split brain, encrypted transport, monitored replication lag,
and continuous WAL archiving for point-in-time recovery. The acknowledged
commit durability policy must be selected from measured latency and the
approved data-loss objective; bank enforcement must not advertise an RPO that
the selected synchronous or asynchronous replication mode cannot meet.

Connection pools need bounded acquisition, statement, transaction, and idle
timeouts. The service must classify transient failover errors separately from
authorization, policy, quota, and provider errors. Read replicas may serve only
operations whose consistency contract tolerates their measured lag; policy
activation, revocation, approval consumption, accounting, audit, and
read-after-write verification remain primary/strong-consistency operations.

The repository Kubernetes manifest points to an external database and is not
evidence of PostgreSQL replication, fencing, WAL archive, or failover.

### Redis

Redis may use replication and failover to improve rate-limit and coordination
availability, but the correctness design must tolerate total Redis loss,
eviction, replay, and stale replicas. Cache keys require tenant and revision
namespaces plus bounded TTLs. Locks require bounded leases and fencing where a
stale holder could mutate authoritative state. Restoring a Redis snapshot must
never roll back a PostgreSQL governance decision.

## Backup and Key-Separation Target

The production backup set must include:

- PostgreSQL base backups plus continuous WAL needed for the approved PITR
  window;
- schema, migration history, functions, policies, roles, grants, and extensions
  needed to reconstruct the isolation boundary;
- all authoritative governance, accounting, audit, outbox, retention, and
  legal-hold rows;
- a signed or otherwise integrity-protected manifest with artifact digests,
  database timeline, recovery coordinates, schema version, creation result,
  and retention class;
- separately retained release images and configuration needed to run a
  compatible restore verifier.

Backups must be encrypted in transit and at rest. Database/storage encryption,
backup-envelope encryption, audit-anchor signing, and application secret
management must use separated key purposes and access policies. Backup
operators should be able to create and retain encrypted artifacts without
being able to decrypt production secrets or alter audit anchors. Restore-key
access must be logged and subject to the environment's approved dual-control
or break-glass process.

Encryption keys must not be stored in database dumps, backup manifests,
container images, ConfigMaps, CI artifacts, or this repository. Key rotation
must retain the protected old key versions for the entire backup-retention
window or re-wrap affected backups safely. Losing a current application key
must not require deleting older backup keys, and deleting an expired backup key
must follow verified retention and legal-hold checks.

At least one backup copy should be immutable against the production workload
identity and isolated from a production-account compromise. Cross-region or
cross-account copies, retention duration, object-lock mode, and deletion
approval are environment-specific controls that require documented threat and
legal review. Their existence is not proven by the repository's local dump
script.

Redis snapshots, local runtime logs, and caches are not substitutes for the
authoritative PostgreSQL backup. Secret values are recovered from their source
secret manager under its own tested recovery plan; only stable `SecretRef`
metadata belongs in PostgreSQL.

## Isolated Restore Verification

Every production-class restore drill should use an isolated account, project,
cluster, or network with fresh restore credentials. It must not contact real
providers, identity providers, webhooks, or SIEM destinations. Use explicit
test endpoints or deny egress while verification runs.

The verifier must perform these steps and record each result:

1. Select a declared disaster cutoff and an eligible backup/PITR coordinate.
   Record artifact, manifest, key-version, release, and schema digests.
2. Provision a clean database and the intended roles without reusing production
   application credentials.
3. Verify artifact integrity before decryption, restore the base backup, replay
   WAL to the chosen recovery coordinate, and reject unexpected timeline or
   checksum changes.
4. Compare migration checksums and schema inventory with the compatible release.
   Do not silently migrate the only restored copy before its original state is
   verified and retained.
5. Verify roles, grants, `ENABLE` plus `FORCE ROW LEVEL SECURITY`, tenant
   policies, connection tenant context, and cross-tenant read and write denial
   for every tenant-owned table.
6. Verify tenant and identity counts; active and revoked sessions; immutable
   policy/classification revisions; approvals and activation history; exactly
   one valid active and last-known-good pointer per governed scope; provider
   registry, routing, capability, credential-reference, and pricing revision
   integrity.
7. Reconcile reservations, usage events, ledger uniqueness, budget aggregates,
   idempotency outcomes, execution-approval consumption, and unknown-commit
   recovery invariants.
8. Recompute the complete audit chain from a trusted pre-backup anchor through
   the latest restored event. Verify event canonicalization, predecessor links,
   sequence rules, tenant boundaries, and anchor signatures without exporting
   tenant content into drill evidence.
9. Verify SIEM outbox and dead-letter counts, delivery IDs, and audit links with
   outbound delivery disabled. Verify retention policies and demonstrate that
   legal-held records cannot be purged.
10. Start compatible application binaries against the isolated database and run
    tenant-scoped readiness and synthetic authorization, policy, reservation,
    audit, and denial checks. Provider invocation must remain stubbed.
11. Record measured recovery values, verifier version, sanitized counts and
    digests, exceptions, operator/approver identities, and the final disposition
    in an access-controlled evidence store.

A successful `pg_restore` or `/readyz` response alone is not a successful
governance restore.

## RPO and RTO Evidence

Each deployment must have approved objectives for at least governance writes,
accounting/audit writes, service availability, and SIEM delivery. This document
does not assign or approve production numbers.

- **RPO** is the interval between the latest acknowledged authoritative commit
  before the declared disaster cutoff and the latest such commit present and
  verified after recovery. It must be measured with committed recovery markers
  or database recovery coordinates, not inferred from dump duration.
- **RTO** starts at the declared incident or recovery start time and ends only
  when the recovered service passes the full acceptance gate and is safe to
  receive its intended traffic. Database restore duration alone is a useful
  component, not the complete service RTO.
- **SIEM delivery recovery** separately measures the oldest undelivered outbox
  age and time to drain the backlog without duplication after service recovery.
- **Audit recovery** separately records the newest verified audit sequence and
  trusted anchor on the restored timeline.

Evidence must include the approved thresholds, observed values, timestamps,
clock source, recovery coordinates, data markers, excluded downtime, failed
attempts, and pass/fail calculation. It must be written atomically, redacted,
integrity protected, retained outside the failed system, and linked to the
change or incident record. A configurable CI default is test scaffolding and
must not be presented as the deployment's approved objective.

## Failure Runbooks

The following are target runbook skeletons. Environment-specific endpoints,
roles, escalation contacts, commands, and approval paths must be completed and
tested outside this repository without committing credentials or tenant data.

### PostgreSQL primary or zone loss

1. Declare the incident, stop automated changes, and capture primary/replica
   timeline, lag, pool, and error evidence.
2. Fence the failed writer before promotion. Do not permit two writable
   primaries.
3. Allow the database service or approved operator to promote the eligible
   replica, then refresh pools and reject connections to the old timeline.
4. Verify schema compatibility, forced RLS, active policy pointers, recent
   revocations, ledger uniqueness, audit continuity, and outbox progress.
5. Reopen traffic gradually. Reconcile unknown commit outcomes through stable
   idempotency keys rather than replaying provider calls blindly.

### Database corruption, destructive change, or failed migration

1. Freeze mutations and preserve forensic copies, logs, migration history, and
   recovery coordinates.
2. Determine whether a reviewed forward fix is safer than PITR. Never edit
   immutable policy, ledger, approval, or audit rows to conceal the failure.
3. Restore to an isolated database immediately before the harmful transaction,
   run the full verifier, and obtain cutover approval.
4. Fence the damaged database, switch credentials or endpoints through the
   secret manager, monitor, and retain rollback evidence until closure.

### Region or control-plane outage

1. Prevent split-brain activation and approval processing in the failed region.
2. Promote an already eligible protected replica or perform PITR from the
   isolated backup copy using the approved recovery coordinate.
3. Re-establish identity, policy, secret-manager, audit, and SIEM dependencies;
   do not expose the data plane merely because health probes respond.
4. Run the complete acceptance gate, update traffic, and measure the full RPO
   and RTO from the declared cutoff.

### Redis loss or stale failover

1. Treat Redis data as disposable; do not copy it into PostgreSQL as recovered
   truth.
2. Enter the documented conservative admission/rate-limit behavior, replace or
   flush the cluster, and rebuild revisioned caches from PostgreSQL.
3. Verify no budget, approval, active policy, audit, or ledger record changed as
   a result. Reopen capacity gradually and measure overshoot or false-denial
   bounds.

### Audit chain, SIEM outbox, or legal-hold failure

1. Stop operations identified as mandatory-audit or approval-sensitive by the
   failure matrix; do not acknowledge success and discard the audit failure.
2. Preserve the last trusted anchor, affected rows, outbox state, diagnostics,
   and current retention/hold decisions.
3. Repair through an auditable, append-only procedure. Never rewrite digests or
   delete dead letters to make chain verification pass.
4. Verify chain continuity and idempotent SIEM delivery before resuming. Record
   any integrity gap explicitly and escalate it through the incident process.

### Backup artifact or decryption-key failure

1. Quarantine the artifact without deleting other generations; validate its
   manifest and replicas.
2. Invoke the approved restore-key access process. Never bypass encryption by
   placing a key in an environment file, command line, ticket, or CI log.
3. Try the next eligible immutable recovery point and recalculate the achievable
   RPO. Notify the incident owner when the objective can no longer be met.
4. After recovery, remediate backup monitoring, key retention, and independent
   copy controls before declaring closure.

## Required Drill Matrix

| Drill | Minimum target verification | Evidence required |
| --- | --- | --- |
| PostgreSQL replica failover | Writer fencing, bounded service interruption, tenant isolation, no duplicate commits, audit/outbox continuity | Timeline and lag, fault timestamps, request outcomes, RPO/RTO calculation |
| Isolated PITR | Chosen recovery coordinate, full governance schema, policy/registry consistency, audit chain, RLS, legal hold | Manifest and schema digests, sanitized invariant results, approved disposition |
| Migration failure | Locking, partial-phase detection, forward fix or rollback, mixed-version compatibility | Migration checksums, lock/lag measurements, before/after verifier results |
| Redis total loss | Conservative behavior and rebuild without durable-state loss | Admission/rate-limit results, PostgreSQL before/after fingerprints |
| SIEM outage and recovery | Transactional buffering, bounded retry, dead letter, idempotent drain | Oldest-event age, backlog size, duplicate count, signed delivery acknowledgements |
| Backup-key rotation | Restore old and new generations under separated access | Key-version references, access audit links, restore verifier result |
| Region loss | Split-brain prevention, dependency recovery, controlled cutover | Incident timeline, traffic change record, full RPO/RTO and acceptance gate |
| Legal-hold recovery | Held records survive backup, restore, retention, and purge attempts | Hold identifiers/digests, denied-purge event, audit-chain verification |

Cadence, environment coverage, responsible owner, witness requirements, and
evidence retention must be approved in the operating environment. A tabletop
exercise cannot replace the scheduled technical restore, failover, and cutover
drills. Failed or stale drills must surface as operational alerts and block any
readiness assertion whose evidence requirements they no longer satisfy.

## Exit Criteria

Storage and recovery work is not complete until all of the following have
reviewed implementation and current environment evidence:

- PostgreSQL contains every required governance entity and is the enforced
  authority for enterprise multi-replica modes.
- Redis-backed authoritative state is rejected, including compatibility paths.
- All tenant tables use forced RLS with separated roles and exhaustive schema
  and cross-tenant tests.
- Migration artifacts and tooling meet the external-only,
  expand/backfill/contract, compatibility, and recovery rules.
- Gateway, control plane, audit exporter, PostgreSQL, and required dependencies
  meet the approved HA topology and have demonstrated failover behavior.
- Backups are encrypted, immutable where required, key-separated, monitored,
  and restorable without production credentials or external provider traffic.
- The isolated verifier covers policy revisions and pointers, classification,
  approvals, provider registry/routing/pricing, sessions and revocations,
  accounting, audit chain and anchors, SIEM state, retention, and legal holds.
- RPO and RTO are approved per environment and supported by recent, reproducible
  end-to-end drill evidence rather than CI defaults.
- Every failure runbook has named operational ownership, tested commands,
  escalation paths, and retained evidence with unresolved failures tracked in
  the implementation ledger.
