# Rollout, Rollback, and Deprecation

## Purpose and Current State

This document defines a reversible path from the current compatibility runtime
to one governed production pipeline. It does not authorize a rollout or claim
that the named governance modes are fully implemented.

Existing repository foundations include immutable/LKG-style runtime policy
snapshots, API version/deprecation domain decisions, compatibility and
deprecation metric plans, strict gateway routes, bounded drain plans, a
three-replica Kubernetes gateway scaffold, external migration modes, provider
affinity and pre-commit rotation invariants, and historical compatibility/load
tests. Material gaps remain: the typed governance modes, complete policy and
registry lifecycle, tenant cohort controller, unified access sessions,
bank-mode startup gates, current governance benchmarks, and removal of all
anonymous or early-dispatch bypasses.

Rollout is therefore a controlled migration of authority, not a collection of
independent feature toggles.

## Non-Negotiable Invariants

Every rollout and rollback must preserve:

- no cross-tenant access or ambiguous tenant context;
- no raw content, secret, token, detector match, or full IP in operational
  surfaces or ordinary governance storage;
- no implicit public classification, allow policy, provider registry, or raw
  secret fallback in an enforcement mode;
- no provider outside the original hard eligible set;
- continuation affinity and no retry/rotation after stream commit;
- bounded tasks, queues, retries, findings, rules, sessions, candidates,
  metrics, and caches;
- PostgreSQL authority and RLS for enterprise durable state; Redis remains
  rebuildable coordination;
- mandatory audit and approval failure behavior for the selected mode;
- compatibility with upstream Codex transport and errors unless the local
  gateway fails before any upstream response exists;
- no terminal output while the Codex TUI is active.

Rollback may restore an earlier approved immutable policy, classification,
registry, score, or application revision. It may not disable a mandatory bank
control, select an unapproved provider, resurrect a revoked credential, lower
retained classification, discard audit history, or replay a committed stream.

## Rollout Control Model

Rollout state must be typed, versioned, tenant-scoped, immutable after
publication, approved where required, and auditable. A data-plane request reads
one coherent snapshot containing at least:

- governance mode and capability rollout states;
- tenant/cohort assignment revision;
- classification and inspection revision;
- policy active and LKG revisions;
- provider registry, routing score, and pricing revisions;
- session and affinity revision behavior;
- mandatory audit and secret-provider requirements;
- activation time, expiry for temporary states, and stable fingerprint.

Cohort assignment uses stable bounded identifiers and deterministic selection.
It must not hash raw prompt content, full user identity, or arbitrary provider
names. Operators cannot change a cohort by setting an untracked environment
variable on one replica.

The allowed progression for an individual capability is:

```text
off -> observe -> enforce -> retired-legacy
```

`bank_enforce` is a deployment profile with stricter startup and runtime gates,
not merely the final percentage in a canary. A transition that weakens a bank
control requires narrow break-glass authorization, a reason, an expiry, and
post-event review; some controls, such as tenant isolation, cannot be waived.

## Five-Phase Migration

### Phase 0: baseline and inventory

Required before behavior changes:

1. Pin a clean source revision and record environment/toolchain.
2. Inventory every channel, route, body schema, stream, compatibility adapter,
   session/affinity path, provider, policy, audit, secret, and store.
3. Run formatting, Clippy, tests, boundary/deployment guards, compatibility,
   load, benchmark, and storage/recovery baselines; record failures and
   blockers exactly.
4. Establish the implementation ledger and architecture/data-flow documents.
5. Freeze new bypasses with CI architecture guards.

Rollback is source-only because no production authority has changed. Exit
requires an accepted baseline, known gaps, and named owners.

### Phase 1: inspection boundary

1. Add one bounded typed inspection result in the domain/application path.
2. Adapt Presidio and local detectors behind that boundary.
3. Observe coverage, classification floor, findings category, latency, and
   failures without changing routing.
4. Compare every channel/schema, including tool arguments and WebSocket paths.
5. Enforce only after shadow mismatches are explained and unsupported media has
   explicit policy behavior.
6. Remove duplicate authoritative PII policy only after convergence is proven.

Rollback activates the prior inspection snapshot or returns an eligible cohort
to observe/off according to deployment mode. It does not remove masking already
required by policy or reinterpret an inspection failure as public data.

### Phase 2: classification and bidirectional guardrails

1. Publish immutable classification rules with checksum, validation, approval
   where required, active/LKG pointers, and rollback.
2. Shadow request/response classification and obligation merge.
3. Enforce masking, tools/models/modalities/tokens/locality/retention controls
   on the smallest tenant cohort.
4. Expand through public, internal, confidential, restricted, partial, and
   unsupported coverage cases across unary/SSE/WebSocket.
5. Prove pre-commit denial, post-commit termination/accounting, and session
   classification monotonicity.

Rollback activates the preceding approved classification revision or returns a
cohort to observe where the deployment mode permits. Retained classification
does not decrease, and restricted data still cannot route to an ineligible
provider.

### Phase 3: PDP, approvals, audit, and SIEM outbox

1. Publish the pure PDP and compiled snapshot in observe mode beside legacy
   decision telemetry, with no second production authority.
2. Require validate/analyze/approve/activate/audit for immutable revisions.
3. Shadow explicit effects, obligations, reasons, and approval requirements.
4. Cut each policy enforcement point to the one application decision in a
   reversible cohort.
5. Make control-plane activation, approval, durable audit, and outbox behavior
   transactional where required.
6. Prove audit-chain verification and remote SIEM outage independence.
7. Remove legacy policy authority after parity, soak, and rollback drills.

Rollback activates a prior approved immutable policy/LKG revision. It never
changes history or silently chooses an allow-all policy. If no valid bank
snapshot exists, new work fails closed.

### Phase 4: governed provider routing

1. Publish a revisioned approved registry with `SecretRef` credentials.
2. Shadow hard eligibility and deterministic fixed-point score decisions while
   the legacy selector still dispatches.
3. Block any observed hard-compliance mismatch before enabling governed
   dispatch.
4. Enable hard filters first, then deterministic score, bounded pre-commit
   fallback, accounting revision, and continuation pinning.
5. Canary by tenant/cohort and provider capability; monitor eligible-set size,
   mismatch, fallback, circuit, cost, quota, load, and affinity.
6. Remove legacy routing authority after parity and outage/DR drills.

Rollback atomically returns a cohort to the prior approved registry and score
revision or, before legacy retirement, to the prior selector only if the same
hard compliance filter remains authoritative. Active committed streams stay on
their provider. Continuations honor their pinned policy unless an emergency
provider revocation requires pre-commit denial.

### Phase 5: unified gateway and bank profile

1. Converge CLI, IDE, browser/API, service, compact, SSE, and WebSocket entry
   points on the authenticated application use case.
2. Enable exact OIDC/bearer/service identity, trusted proxy, access-session, and
   mTLS controls in staged internal cohorts.
3. Enforce PostgreSQL authority/RLS, external secret provider, durable audit and
   SIEM outbox, private exposure, restricted egress, and multi-replica state.
4. Run bank fail-closed, deployment, chaos, backup/restore, audit-integrity,
   failover, and DR gates.
5. Move production traffic only after current SLO/capacity evidence and a
   practiced rollback exist.
6. Retire anonymous and file/Redis-authoritative enterprise compatibility paths.

Application rollback uses the previous compatible image and immutable
snapshots after verifying schema compatibility. A bank deployment cannot roll
back to a release that lacks its mandatory identity, tenant, secret, audit, or
storage controls.

## Promotion Gates

Every cohort promotion requires all applicable rows below to pass on the exact
candidate build and revisions.

| Gate | Minimum evidence |
| --- | --- |
| Architecture | Every enabled channel reaches the same application stage; no adapter owns authz/policy; no synchronous request-path revision lookup or DDL |
| Security | Negative tenant, scope, identity, inspection, policy, routing, secret, audit, proxy, smuggling, and content-leak tests pass |
| Correctness | No unexplained shadow mismatch; deterministic decisions; required audit/reconciliation links; compatibility contract pass |
| Performance | Five comparable samples meet the accepted budgets for captured metrics; unsupported metrics remain explicit |
| Reliability | Bounded saturation, slow client/upstream, cancellation, provider outage, LKG, Redis loss, and database failure tests pass |
| Operations | Signals are live, cardinality bounded, alerts synthetically fire, runbooks and owners exist, error budget has headroom |
| Data | Expand/backfill validation passes; RLS and cross-tenant tests pass; current backup and isolated restore evidence exists |
| Rollback | Previous image/snapshots remain compatible; rollback and forward-fix have been rehearsed; no active incompatible stream/session is abandoned |

Promotion pauses automatically on an invariant violation, unexplained mismatch,
error-budget burn, audit/secret failure, eligible-set escape, database
isolation failure, or evidence staleness.

## Deployment Choreography

For each production promotion:

1. Approve the candidate source, image digest, migrations, immutable
   configuration/policy/registry digests, cohort, and change window.
2. Verify current encrypted backup and isolated restore evidence. Record the
   rollback schema compatibility window.
3. Run external expand migrations with the migration identity. Gate on lock,
   capacity, replication lag, checksum, and mixed-version compatibility.
4. Deploy canary replicas with no public exposure, approved secret references,
   readiness disabled until snapshots and dependencies satisfy the mode.
5. Exercise synthetic allow/deny, restricted-locality, identity, accounting,
   audit/outbox, cancellation, and stream-drain checks.
6. Shift a bounded cohort. Observe at least one approved soak interval and all
   fast/slow burn alerts.
7. Increase cohorts monotonically while retaining the previous compatible
   replicas and immutable snapshots.
8. Drain old replicas: readiness fails first, new admission stops, unary and
   streams finish within bounds, and no mid-stream rotation occurs.
9. Mark completion only after replica convergence, reconciliation, audit/SIEM,
   and SLO evidence. Do not contract schema in the same change.

The existing Kubernetes pre-stop delay, termination grace, topology spread,
and disruption budget are scaffolding; environment tests must prove their
actual timing and active-stream behavior.

## Rollback Decision and Procedure

### Immediate rollback triggers

- cross-tenant success or confused-deputy authorization;
- restricted or otherwise ineligible provider selection/fallback;
- invalid/missing mandatory audit for an acknowledged operation;
- secret, token, raw content, detector match, or full IP leakage;
- policy/registry divergence or activation without required approval/audit;
- irreversible accounting duplication or unexplained provider invocation;
- sustained severe error-budget burn, deadlock/unbounded growth, or failure to
  drain;
- migration/RLS failure or evidence of authoritative-state loss.

### Safe rollback steps

1. Stop further cohort expansion and control-plane publication.
2. Preserve redacted telemetry, audit anchors, revision digests, migration
   state, database recovery coordinates, and incident timeline.
3. Stop new admission for affected cohorts. Let committed streams finish or
   terminate according to their admitted obligation; never replay or rotate
   them mid-stream.
4. Activate the last approved compatible policy/classification/registry/score
   snapshot, or deploy the prior compatible image. Keep hard compliance and
   bank mandatory controls active.
5. For database changes, prefer a reviewed forward fix after an expand
   migration. Use PITR only under the recovery plan; never erase audit,
   approval, ledger, legal-hold, or outbox history through a generic down
   migration.
6. Verify identity, tenant/RLS, classification, policy, eligible routing,
   accounting, audit chain/outbox, secret references, and readiness before
   reopening traffic.
7. Reconcile unknown commits and reservations by idempotency key. Do not blindly
   retry provider work.
8. Record disposition, affected cohorts/revisions, objective impact, follow-up
   owner, and the conditions required before another promotion.

### Rollback matrix

| Failed component | Preferred rollback | Forbidden shortcut |
| --- | --- | --- |
| Inspection/classification revision | Activate prior approved revision or observe state allowed by deployment mode | Treat unknown/unsupported as public |
| Policy revision | Activate prior approved/LKG immutable revision | Allow-all or process-local bypass |
| Provider registry/score | Activate prior approved registry/score; keep hard filters | Legacy selector without compliance filtering; cross-eligible-set fallback |
| Identity/session change | Revert to prior compatible verifier/session schema while preserving revocations | Accept anonymous enterprise traffic or raw forwarding headers |
| Application image | Drain candidate and deploy prior schema-compatible digest | Kill/replay active streams or run request-path migration |
| Expand migration | Leave additive schema and deploy compatible prior image | Destructive down migration that removes durable history |
| Bad data mutation | Freeze writes; reviewed forward fix or isolated PITR/cutover | Manual edits to audit/ledger/approval history |
| SIEM exporter | Stop exporter, retain durable outbox, deploy prior exporter | Block data path on remote SIEM or drop backlog |
| Vault adapter | Use still-valid prior lease/version under policy, or deny | Copy secret into env/argv/config in bank mode |

## Schema and State Compatibility

Schema evolution follows expand -> bounded backfill -> validation -> cutover ->
contract. Expand and contract do not ship in one release. At least one supported
rollback boundary must read the expanded shape before legacy columns/tables are
removed.

Before contracting compatibility state:

- scan for unowned/null tenant data and resolve it deterministically;
- prove old/new mixed-version reads and writes where rolling deployment needs
  them;
- verify tenant-aware keys, foreign keys, idempotency, forced RLS target, roles,
  and transaction-local tenant context;
- verify policy/registry/session/audit/outbox and accounting recovery;
- retain an export or forward-recovery path for destructive change;
- confirm no supported rollback target or retained session depends on the old
  representation.

Redis keys and local caches are version/revision namespaced and disposable.
They are invalidated or rebuilt after rollback; they never decide which durable
revision is active.

## Compatibility Inventory and Removal Contract

Every temporary compatibility path requires an owner, test, bounded telemetry,
removal condition, and maximum retention release. The current migration must at
least track:

| Compatibility path | Required coverage before removal | Removal condition |
| --- | --- | --- |
| Anonymous local data-plane admission | Personal loopback regression plus all authenticated channel parity tests | Enterprise/bank entry points always carry tenant/principal/session/governance context; personal local contract remains explicit |
| Early Gemini Live or other WebSocket dispatch | Frame/schema inspection, policy, affinity, stream commitment, and accounting tests | Every supported upgrade uses the governed application decision before provider dispatch and frame obligations during streaming |
| Duplicate local PII/redaction policy | Unicode/schema/tool argument/stream tests and mode failure matrix | One application inspection/classification authority covers all supported schemas and fallback behavior |
| Legacy provider/model selector | Shadow parity, hard-filter, score, affinity, outage, fallback, and cost tests | Governed router is sole dispatch authority and rollback uses approved snapshots |
| File/Redis authoritative gateway state | Migration, dual-read if required, RLS, multi-replica, backup/restore tests | Enterprise modes reject it; PostgreSQL owns every durable record |
| Environment/raw secret sources | SecretRef migration and rotation/outage tests | Bank startup rejects them and every adapter resolves through the approved secret provider |
| Compatibility API version | Contract, usage telemetry, deprecation notice, client migration, and sunset test | Approved sunset reached, usage below threshold for the full notice window, and rollback no longer needs the version |

Telemetry for compatibility use must use bounded surface/result labels, never
tenant/user/model strings. A path without an owner or removal condition is not
an acceptable permanent second authority.

## API Deprecation Lifecycle

The repository already models current, deprecated, and sunset API versions and
stable unsupported/sunset errors. Production deprecation must add a controlled
lifecycle around that decision:

1. **Inventory:** identify clients, schemas, semantics, owner, and replacement.
2. **Additive introduction:** publish the replacement OpenAPI/CLI contract and
   compatibility tests before deprecating the old surface.
3. **Notice:** publish a versioned deprecation record with start time, proposed
   sunset, migration guide, support owner, and telemetry. Emit standard
   deprecation/sunset response metadata where the HTTP contract supports it,
   without exposing caller identity.
4. **Migration:** give supported clients a tested overlap window; monitor only
   bounded usage buckets and contact owners through approved operational data.
5. **Sunset approval:** verify migration, error budget, zero mandatory internal
   dependency, rollback independence, and required notice policy. Approval and
   audit are mandatory for enterprise control-plane or bank surfaces.
6. **Rejection:** after the effective sunset, return the stable gone/unsupported
   error. Do not silently route to a different semantic version.
7. **Removal:** delete code/schema only after one further compatible release
   boundary or the approved policy, with tests and telemetry removed in the
   same change.

Security fixes may shorten an ordinary notice period, but the emergency change
still records reason, scope, approval, compatibility impact, and client
remediation. A deprecated path never bypasses current identity, classification,
policy, routing, secret, or audit controls.

## Final Cutover and Exit Criteria

The migration completes only when:

- CLI, IDE, API/browser, services, compact, SSE, WebSocket, and other supported
  channels share one authenticated governed application pipeline;
- inspection/classification, PDP, admission, routing, response guard,
  accounting, and audit have one production authority each;
- every material revision has validation, approval, activation, LKG, rollback,
  audit, and replica-convergence evidence;
- enterprise/bank modes reject anonymous, raw-secret, public-bind,
  unresolved-classification, unapproved-registry, and missing-audit states;
- current benchmark/SLO, security, chaos, migration, multi-replica,
  backup/restore, audit-integrity, and DR gates pass;
- all temporary compatibility paths have either been deleted or retain a
  documented owner, test, telemetry, expiry, and exact blocker;
- rollback drills demonstrate prior-image and prior-snapshot recovery without
  weakening hard controls or violating committed streams.

Until these criteria pass against the final candidate, the rollout remains in
progress and bank readiness must not be claimed.
