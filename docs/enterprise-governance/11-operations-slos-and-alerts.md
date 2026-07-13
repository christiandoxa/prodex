# Operations, SLOs, Alerts, and Runbooks

## Status and Interpretation

This document defines the minimum operational contract for the governed
gateway. Values marked **initial target** are proposed release objectives that
must be approved and measured per environment; they are not claims about an
existing deployment. Existing metric-plan types are not equivalent to a live
exporter, alert rule, dashboard, pager integration, or achieved SLO.

Repository evidence observed on 2026-07-13 includes:

- bounded runtime broker Prometheus output for request/lane pressure,
  continuation state, profile health, backoff, allocation when enabled, queue
  waits, and runtime-state lock waits;
- `prodex-observability` plans for API RED signals, authn/authz, tenant
  isolation, policy/JWKS/config refresh, provider/circuit/routing, accounting,
  audit chain, secret rotation, backup/restore, rollout, migration, health, and
  shutdown events;
- runtime logs plus `prodex doctor --runtime` diagnostics for proxy selection,
  admission pressure, transport failures, affinity, and state-save behavior;
- Kubernetes health probes, bounded resources, multiple gateway replicas,
  topology spreading, a disruption budget, and graceful termination
  scaffolding;
- a PostgreSQL backup/restore drill script for the current storage subset.

Material gaps include live end-to-end emission of several planned metric
families, governance inspection/PDP/approval/SIEM/Vault metrics, checked-in
alert rules and dashboards, durable outbox exporter health, production RPO/RTO
evidence, and an approved pager/runbook ownership map.

## Telemetry Safety Contract

Operational telemetry must never contain raw prompt or response content, PII,
secrets, credentials, tokens, detector matches, certificate contents, full IP
addresses, tenant display names, arbitrary provider/model strings, URLs with
query data, or unbounded error messages.

Permitted metric dimensions are closed enums or bounded approved IDs, for
example:

- deployment mode and rollout state;
- route class and channel class;
- stable outcome and reason code;
- classification and inspection coverage enums;
- policy/registry revision age bucket, not a raw revision string on every
  series;
- provider class or bounded registry slot, not an arbitrary endpoint/model;
- circuit state, admission lane, status class, and latency bucket;
- secret purpose and lease-state enum, not secret path or provider name;
- database/Redis role and saturation bucket;
- backup/drill result and age bucket.

Tenant- or principal-level diagnosis belongs in access-controlled, redacted,
bounded event/audit queries keyed by opaque IDs, not metric labels. New labels
require a cardinality budget, owner, maximum values, and a CI guard.

Logs are structured and bounded. During the Codex TUI, runtime notices go only
to the configured runtime log; stdout and stderr remain untouched. Trace spans
carry correlation IDs and stable stage/outcome metadata, not request content.

## Minimum Signal Catalog

The following signals are required before bank-mode readiness can be asserted.
Names are conceptual unless the repository already exports the named metric.

| Control area | Required measures | Required alerts |
| --- | --- | --- |
| Authentication | Attempts, success/failure by stable reason, JWKS age, refresh failure, LKG use, token-validation latency, mTLS verification result | Identity unavailable; key set beyond stale window; unusual bounded failure-rate increase; mTLS trust/expiry failure |
| Authorization and sessions | Allow/deny by stable reason, scope mismatch, tenant-isolation denial, session creation/expiry/revocation/replay/concurrency, step-up required | Cross-tenant invariant failure; revocation propagation stale; session store unavailable; replay surge |
| Inspection/classification | Duration, coverage, finding category, mask/deny, timeout, adapter error, classification, unknown/partial outcome | Required inspection unavailable; unresolved classification in enforce mode; finding-limit/timeout surge |
| Policy/control plane | Decision latency, active revision age, refresh failure, LKG use/age, publish/approval/activation/rollback, cache invalidation | No valid policy snapshot; LKG near/over limit; activation divergence; approval queue oldest age; break-glass use |
| Routing/providers | Eligible candidate count, selected bounded provider class, hard-filter reasons, circuit state, health, fallback, quota/load, score/cost variance | Empty eligible set; prohibited fallback attempt; circuit/open-provider fleet degradation; material cost variance |
| Admission/API | Request count, duration, throughput, status class, body/header rejection, global/lane/tenant denial, queue wait/depth, rate/quota denial, cancellation, drain | Error-budget burn; sustained saturation; queue/lock wait; slow-client pressure; drain timeout |
| Streaming | Time to first upstream/local chunk, active streams, cancellation, read failure, post-commit termination, reconciliation outcome | First-chunk latency burn; stream error increase; active streams not draining |
| Accounting | Reservation, reconciliation, ledger, idempotency, recovery, budget and quota correctness | Reservation/audit transaction failure; reconciliation backlog; duplicate/idempotency conflict; budget invariant failure |
| Audit/SIEM | Durable append, chain verification, gap/digest failure, outbox backlog/oldest age, retry, dead letter, exporter result | Mandatory audit write failure; chain tamper/gap; outbox lag; dead letter; exporter credential/TLS failure |
| Secrets/Vault | Resolve, lease age, renewal, expiry, rotation, stale version, purpose mismatch | Lease near expiry; renewal/rotation failure; unresolved bank secret; use after safe expiry |
| Storage | PostgreSQL pool/transaction/replication/lock saturation, RLS/context failure; Redis availability/latency/eviction and conservative mode | Database unavailable/failover; replication lag exceeds policy; tenant-context failure; Redis loss and conservative-mode duration |
| Deployment/recovery | Readiness, replica availability, restart/eviction, rollout/drain, backup age/result, restore-drill age/result, measured RPO/RTO | Availability burn; rollout regression; stale/failed backup; stale/failed restore drill; objective breach |

Every alert must link to a runnable runbook, name an owning team and escalation
path outside this public repository, specify severity, threshold/window, auto-
resolution behavior, and the evidence needed for closure.

## Initial SLO Contract

These are minimum initial targets for a production-class governed deployment.
An environment may adopt stricter objectives. It may adopt different numeric
values only through an approved, documented risk decision backed by measured
capacity and dependency objectives.

| Service indicator | Initial target | Measurement boundary |
| --- | --- | --- |
| Gateway availability | 99.9% successful eligible requests over 30 days, excluding approved maintenance; security denials are not availability failures | From accepted connection through locally committed success or correct provider result; dependency failures count unless explicitly excluded by the approved contract |
| Control-plane mutation availability | 99.9% over 30 days for valid, authorized mutations | Includes required approval, transaction, audit, and outbox commit |
| Governance-disabled performance | No more than 5% p95/p99 latency or throughput regression against the accepted comparable baseline | Same build profile, load, topology, and five-sample method |
| In-memory governance overhead | p99 added latency at most 5 ms at documented reference load and maximum bounded configuration, excluding external detector/provider time | Classification, PDP, obligation merge, and routing combined |
| PDP decision latency | p99 at most 1 ms for the documented representative policy set | Pure evaluator only; no I/O |
| Routing decision latency | p99 at most 2 ms for the documented representative provider set | Pure hard filter and deterministic score only; no probes/I/O |
| Required inspection availability | 99.9% within its configured stage budget over 30 days | External detector latency is separate; partial/unsupported is success only when policy permits it |
| Policy/registry snapshot readiness | 100% of admitted enforcement requests use a valid active or permitted LKG snapshot | Any request admitted with no valid snapshot is an invariant breach |
| Tenant isolation | 100%; zero cross-tenant access or ambiguous tenant context | Security invariant, not an error-budget trade |
| Mandatory audit durability | 100% of acknowledged mandatory operations have their required durable audit/outbox state | Security invariant; remote SIEM delivery is asynchronous |
| SIEM delivery | Initial target: 99% delivered within 5 minutes and 99.9% within 30 minutes, with zero silent drops | Measured from durable outbox commit; environment must approve actual values |
| Session revocation propagation | Initial target: 99.9% visible to all serving replicas within 30 seconds; never beyond the configured maximum | Measured from authoritative commit to denial on each replica |
| Backup/restore | Meet the environment-approved RPO/RTO and drill cadence | Full isolated restore acceptance, not `pg_restore` or readiness alone |

Security, tenant isolation, mandatory audit, no-mid-stream rotation, and bounded
backpressure have no error budget. A throughput gain cannot compensate for
their regression.

## Burn-Rate and Threshold Policy

Availability and latency alerts should use at least a fast and a slow burn
window to reduce noise while catching severe incidents. Initial routing:

- page when a security invariant is violated once, mandatory audit cannot
  commit, no valid enforcement snapshot exists, audit-chain verification finds
  a gap/digest failure, or a bank secret expires;
- page on fast error-budget burn, sustained total unavailability, cross-tenant
  evidence, or data-loss/recovery-objective breach;
- page or high-priority ticket on sustained admission saturation, revocation
  propagation beyond its maximum, SIEM dead letters, or restore-drill failure;
- ticket on slow error-budget burn, stale backup/drill evidence, rising LKG use,
  capacity headroom loss, or cost variance outside approved bounds;
- record informational events for expected bounded fallback, rollout notices,
  or transient refresh retries that remain within policy.

Exact windows and thresholds belong in versioned environment alert rules. They
must be tested with synthetic signals and reviewed after incidents; this
document does not claim those rules are checked in today.

## Operator Entry Points

Safe first-line evidence sources are:

```bash
prodex doctor --runtime
prodex doctor --runtime --json
prodex quota --all --once
```

The runtime doctor resolves the effective runtime log and summarizes known
pressure, affinity, transport, and persistence markers. Operators should inspect
the latest runtime log pointer rather than changing proxy behavior from a
single symptom. Health, metrics, audit-integrity, policy/registry snapshot, SIEM
outbox, secret-lease, and backup/drill endpoints or commands are target work
where no production operator surface yet exists.

Operator output must be content-free, access controlled where sensitive, and
bounded. A health endpoint must not expose raw configuration, tenant names,
provider endpoints, credentials, prompts, or audit payloads.

## Incident Runbooks

The steps below are repository-safe skeletons. Deployment owners must supply
actual contacts, access roles, environment commands, and approval records in an
access-controlled runbook system.

### Identity provider or JWKS failure

1. Confirm issuer/audience and bounded key-cache age without printing tokens or
   key material.
2. Distinguish IdP outage, DNS/TLS failure, rejected redirect/origin, malformed
   discovery, and local refresh backoff.
3. Keep using LKG keys only within the configured window. Do not extend the
   window with an ad hoc process flag in bank mode.
4. Deny new verification after safe expiry; preserve already authenticated
   sessions only as their explicit policy permits.
5. Restore the approved IdP path, verify new and revoked credentials, then
   close with key-age and negative-test evidence.

### Authentication, authorization, or tenant-isolation anomaly

1. Treat a cross-tenant success as a security incident; stop affected traffic
   and preserve redacted correlation, audit, policy, and database evidence.
2. Verify credential scope, tenant context, role revision, session binding,
   trusted-proxy source, and RLS role without replaying the sensitive request.
3. Revoke affected credentials/sessions and invalidate caches from the
   authoritative store.
4. Resume only after negative cross-tenant, confused-deputy, and replay tests
   pass on the candidate fix.

### Inspection or classification service failure

1. Check adapter timeout/concurrency, response bounds, endpoint/TLS identity,
   coverage, and failure category; do not log detector matches.
2. In observe mode, preserve the recorded unsupported/partial result. In
   enforcement modes, follow the published failure matrix; bank-required
   inspection denies before routing.
3. Do not bypass inspection by switching channel, alias, adapter, or media
   shape. Do not silently classify unknown input as public.
4. Restore capacity, test Unicode, nested tool arguments, malformed/oversized
   responses, and fail-closed behavior before clearing the incident.

### Policy or provider-registry refresh failure

1. Record active, LKG, and candidate revision digests and ages without dumping
   policy source or tenant content.
2. Reject a malformed or unapproved candidate; continue only with a still-valid
   LKG within its bounded window.
3. When no valid enforcement snapshot remains, deny new work. Do not fall back
   to an implicit allow policy or unapproved provider catalog.
4. Repair through the normal validate/approve/activate transaction or activate
   a previously approved immutable revision. Verify replica convergence.

### Provider outage or degraded eligible set

1. Inspect bounded eligibility reasons, endpoint circuit state, transport
   backoff, quota classification, load, and continuation ownership.
2. Retry or fall back only before commit and only within the original hard
   eligible set. Never override continuation affinity with a health heuristic.
3. Do not treat a generic 429 as account quota without an explicit upstream
   quota/rate-limit code.
4. If the eligible set is empty, return the stable pre-commit unavailable
   response. Restore traffic gradually and reconcile reservations.

### Mandatory audit or SIEM failure

1. If the durable local append/outbox transaction fails, stop operations marked
   mandatory; do not acknowledge and drop the event.
2. If only the remote SIEM is unavailable, keep the data path independent while
   bounded durable outbox retry and dead-letter policy operate.
3. Preserve the last trusted audit anchor, backlog age/count, delivery IDs, and
   bounded diagnostics. Never rewrite chain history or delete dead letters to
   hide a failure.
4. Verify the chain, idempotent delivery, and backlog drain rate before
   resuming blocked control-plane operations.

### Vault or external secret-provider failure

1. Identify the affected secret purpose and lease state without exposing its
   path, value, token, or provider endpoint in public telemetry.
2. Continue only while a cached lease remains valid for its purpose and policy.
   Bank mode denies use after safe expiry.
3. Do not copy a secret into environment configuration, argv, a ticket, or a
   ConfigMap as a workaround.
4. Restore TLS/namespace/lease access, rotate if compromise is suspected, and
   prove old/new version behavior before closure.

### PostgreSQL failure or failover

1. Stop automated mutations, capture timeline/lag/pool evidence, and fence the
   old writer before promotion.
2. Verify schema compatibility, transaction-local tenant context, forced RLS
   target, active/LKG pointers, revocations, idempotency, ledger, audit chain,
   and outbox continuity.
3. Reconcile unknown commits by stable idempotency key; never blindly replay a
   provider invocation or approval consumption.
4. Reopen traffic gradually. Follow the complete recovery procedure in
   `09-storage-ha-backup-and-dr.md` for corruption or PITR.

### Redis loss or saturation

1. Treat Redis as disposable coordination, never restored authority.
2. Enter the documented conservative rate/admission behavior; flush or replace
   stale state and rebuild revisioned caches from PostgreSQL.
3. Verify that policy, registry, sessions/revocations, approvals, budgets,
   ledger, and audit authority did not change.
4. Measure false-denial or overshoot bounds and reopen capacity gradually.

### Local admission or queue pressure

1. Inspect `runtime_proxy_active_limit_reached`,
   `runtime_proxy_lane_limit_reached`, `profile_inflight_saturated`, queue-wait,
   and active-limit metrics before blaming upstream transport.
2. Use the lane value to distinguish main `responses` starvation from compact,
   WebSocket, or other-unary pressure.
3. Preserve bounded admission and hard affinity. Do not raise limits beyond
   measured CPU, memory, database, and provider capacity during the incident.
4. Shed new work with stable overload responses and verify drain/recovery.

### Backup, restore, or drill failure

1. Quarantine failed artifacts without deleting other generations and verify
   manifest, key-reference, and storage-copy status.
2. Alert when backup age, restore-drill age, or measured RPO/RTO exceeds the
   approved objective.
3. Restore into an isolated environment with provider/IdP/SIEM egress disabled,
   then run the full governance, RLS, accounting, audit, and outbox verifier.
4. Do not declare recovery from database readiness alone. Follow
   `09-storage-ha-backup-and-dr.md` for the complete acceptance gate.

## Operational Failure Matrix

| Dependency/control | `personal` | `enterprise_observe` | `enterprise_enforce` | `bank_enforce` |
| --- | --- | --- | --- | --- |
| Human identity | Preserve explicit loopback contract | Remote traffic still authenticates; shadow governance may degrade | Deny when required identity cannot be verified | Deny; no remote human access without approved SSO |
| Required inspection | Follow explicit local configuration | Record unsupported/partial and alert | Deny when policy marks it mandatory | Deny unresolved/unsupported classification |
| Policy/registry refresh | Existing security controls remain | Use bounded shadow LKG and alert | Use valid LKG only within policy, then deny | Same, with no implicit allow/default registry |
| Redis | Local behavior as configured | Conservative coordination and alert | Conservative admission; durable authority remains PostgreSQL | Conservative admission; never fail open on security/budget limit |
| Remote SIEM | Not required unless configured | Buffer if durable outbox exists | Continue data plane only when local mandatory audit/outbox committed | Same; block operations whose local durable audit contract cannot commit |
| Secret provider | Existing safe local secret contract | Alert and avoid false success | Deny affected adapter after valid lease expires | Reject raw/env secret fallback and deny after safe expiry |
| PostgreSQL authority | SQLite permitted only by local contract | Enterprise shadow deployment should expose missing authority | Deny operations requiring unavailable durable state | Deny; no Redis/file substitute for authoritative state |

## Readiness and Closure Evidence

Operational readiness requires:

- live emission and scrape tests for every mandatory signal family;
- cardinality and content-leak tests;
- versioned dashboards and alert rules with synthetic firing tests;
- owner, severity, escalation, and runbook link for each alert;
- 30-day or explicitly approved pre-production SLO evidence at representative
  load;
- incident exercises for IdP, inspection, policy/registry, provider, Vault,
  PostgreSQL, Redis, audit/SIEM, slow clients, and cancellation;
- recent backup, isolated restore, audit-integrity, and DR evidence;
- exact commands, timestamps, build/revision IDs, sanitized artifacts, failures,
  and waivers recorded without converting blockers into passes.

Until those artifacts exist and are current, this document is the target
operating contract, not an achieved service-level report.
