# Provider Registry and Governed Routing

## Status and Scope

This document defines the Phase 4 target contract for tenant-aware provider
registration, compliance-first routing, provider execution, and migration from
the current runtime selectors. It is a design and verification baseline, not a
claim that the target is implemented.

The target reuses the existing transport-transparent proxy, continuation
affinity, pre-commit retry, endpoint-aware health, accounting, and provider
translation primitives. It adds one missing authority boundary: every provider
dispatch must be authorized by one immutable, revision-pinned routing decision.

The terms **must**, **must not**, **required**, **should**, and **may** are
normative in this document.

## Invariants

1. Compliance eligibility is evaluated before cost, load, latency, or any other
   preference.
2. One routing planner is authoritative for a request. Shadow planners may
   observe but must not affect dispatch.
3. The planner is pure, bounded, deterministic, and uses only immutable input
   snapshots. It performs no network, disk, secret-store, or health-probe I/O.
4. Provider credentials are represented by `SecretRef`. Raw secret material is
   resolved only after a route is selected and immediately before dispatch.
5. A fallback candidate must have belonged to the request's original eligible
   set. Retry may subtract candidates; it must never add a newly observed one.
6. Retry and rotation stop when a response or stream is committed. There is no
   mid-stream provider change.
7. A continuation preserves valid provider affinity and pinned revisions.
   Explicit policy prohibition and provider revocation take precedence over
   affinity; transient health, load, and backoff signals do not.
8. Upstream status, body, headers, stream behavior, and error semantics remain
   transparent unless Prodex itself fails before an upstream response exists.
9. Decision, candidate, reason, metric, and cache cardinality is configured and
   bounded. Identifiers and diagnostics are redacted before logging or audit.
10. In bank-enforcement mode, missing or invalid policy, registry, pricing,
    classification, audit, or credential-scope state fails closed.

## Provider Identity

Provider identity is separate from adapter identity. Conflating them would make
it impossible to register two deployments that use the same wire protocol but
have different owners, credentials, regions, risk, or prices.

| Identity | Meaning | Example shape |
| --- | --- | --- |
| `AdapterKind` | Code-owned protocol and translation implementation | `openai`, `anthropic`, `gemini`, `openai_compatible` |
| `ProviderInstanceId` | Stable, opaque, tenant-scoped governed resource | `provider_01` |
| `ProviderRevisionId` | Immutable revision of one provider instance | Monotonic revision or opaque version ID |
| `DeploymentId` | Endpoint and region within a provider revision | `region_primary` |
| `ProviderModelId` | Canonical model offered by that instance | Tenant-scoped opaque ID |
| `UpstreamModelName` | Adapter-facing upstream model name | Stored in the approved descriptor |

`AdapterKind` may remain a closed code enum because adapters are executable
code. `ProviderInstanceId` must not be the existing fixed `ProviderId` enum: it
is tenant data, and multiple provider instances may share one adapter.

Opaque IDs, tenant binding, maximum lengths, and allowed character sets are
validated at the domain boundary. Display names and upstream names are data,
not identities. Aliases resolve inside one immutable registry snapshot and may
not cross tenant boundaries.

## Revisioned Provider Descriptor

Each provider revision is immutable and contains no raw secret material. The
domain shape must cover at least:

~~~text
ProviderDescriptorRevision
  tenant_id
  provider_instance_id
  provider_revision_id
  adapter_kind
  lifecycle_status
  display_metadata
  credential_ref: SecretRef
  models[]
    provider_model_id
    upstream_model_name
    aliases[]
    context_window
    maximum_output
    endpoint_capabilities
    capability_status: native | translated | unsupported | untested
  deployments[]
    deployment_id
    endpoint_ref
    regions[]
    data_residency[]
    transport_modes[]
  data_handling
    retention_class
    training_use
    subprocessors_or_trust_domain
  assurance
    trust_tier
    risk_tier
  execution_limits
    connect_timeout
    first_byte_timeout
    idle_timeout
    total_timeout
    per_endpoint_concurrency
  pricing_revision_ref
  quota_policy_ref
  health_policy
  created_by
  created_at
  content_checksum
  approval_record
~~~

Endpoint references are bounded configuration values, never credential-bearing
URLs. Health policy defines probe endpoint, interval, timeout, thresholds,
backoff, and endpoint coupling without embedding authentication material.

Capability status is explicit. `unsupported` and `untested` are ineligible when
a request requires the capability. `translated` is eligible only when policy
permits the translation and its documented semantic limitations.

### Lifecycle and Approval

The minimum lifecycle is:

~~~text
draft -> pending_approval -> approved -> active
                                  |          |
                                  |          +-> suspended -> active
                                  |          +-> revoked
                                  +-> rejected
active or revoked -> retired
~~~

- Draft validation and test probes do not authorize production use.
- Approval uses maker-checker separation, configured quorum, authorization,
  idempotency, and optimistic revision preconditions.
- Activation atomically advances the tenant's active registry pointer and emits
  mandatory audit and outbox records.
- Malformed, unapproved, expired, or unverifiable revisions never replace the
  last-known-good snapshot.
- Suspension prevents new selection but may be reversible. Revocation is an
  explicit security action and is not cleared by a health success.
- Every create, edit, submit, approve, reject, activate, suspend, revoke, and
  retire action records actor, tenant, resource, old and new revision, reason,
  idempotency key, timestamp, and outcome without secret material.

### Immutable Tenant Snapshot

The data plane consumes a precompiled `TenantProviderRegistrySnapshot`:

~~~text
tenant_id
registry_revision_id
activated_at
content_checksum
sorted provider revisions
alias index
capability index
pricing revision references
bounded cardinality metadata
~~~

Publication validates all cross-references and bounds before activation. Each
gateway verifies the checksum, constructs indexes off the request path, and
atomically swaps the last-known-good snapshot. Requests capture one snapshot at
admission and never observe a partially refreshed registry.

Provider revisions remain addressable while pinned continuations may lawfully
refer to them. Retention is bounded by configured age and active pin references.
A missing required pinned revision fails closed; it is not silently replaced by
the current revision.

## Routing Planner Contract

The routing planner accepts values, not services:

~~~text
RoutingInput
  governed request context
  request classification and risk
  policy decision and obligations
  required endpoint and capabilities
  requested model or alias
  continuation affinity and pinned revisions
  tenant provider registry snapshot
  endpoint health and circuit snapshot
  quota snapshot
  pricing snapshot
  bounded load snapshot
  reservation scope and estimated usage
  explicit evaluation time and deadline
~~~

The planner must not read the clock, generate randomness, query a provider,
resolve a secret, reserve money, mutate a circuit, or update affinity. Those
operations occur around the pure plan under application orchestration.

The output is one bounded `RoutingDecision`:

~~~text
routing_decision_id
tenant_id
request_id
selected provider instance, revision, deployment, model, and adapter
ordered original eligible set
selected reason
candidate eligibility reasons
score component breakdown
policy_revision_id
classification_revision_id
registry_revision_id
pricing_revision_id
score_revision_id
health_snapshot_id
quota_snapshot_id
load_snapshot_id
affinity and pin disposition
fallback_set_hash
decision timestamp and expiry
~~~

The application assigns the decision ID and timestamp around the pure result.
The planner returns deterministic decision content for identical inputs.

## Stage 1: Hard Eligibility

Hard eligibility is a conjunction. A candidate that fails one condition is not
scored, even if it is cheaper or healthier. Stable reason codes identify every
failed condition while external responses disclose only authorized detail.

The filter covers:

- tenant and credential-scope binding;
- active, approved provider and deployment revisions;
- revocation, suspension, and activation time;
- provider/model allow and deny obligations;
- request classification, residency, retention, training-use, trust, and risk
  requirements;
- endpoint, model, context-window, output-limit, transport, tool, image, audio,
  schema, and streaming capabilities;
- adapter availability and explicit capability status;
- valid `SecretRef` metadata and permitted data-plane secret scope, without
  resolving the secret;
- hard quota, budget, concurrency, and reservation feasibility;
- fresh-request endpoint circuit and hard admission state;
- deadline and configured timeout feasibility; and
- continuation affinity and pinned revision rules.

Reason codes are stable domain values such as
`provider_not_active`, `provider_revoked`, `policy_denied`,
`residency_mismatch`, `retention_mismatch`, `training_use_prohibited`,
`trust_tier_insufficient`, `risk_tier_exceeded`, `capability_unsupported`,
`context_limit_exceeded`, `credential_scope_mismatch`, `quota_exhausted`,
`budget_unavailable`, `circuit_open`, and `pinned_revision_unavailable`.

For a fresh request, an open circuit, exhausted hard quota, or saturated hard
admission limit may make a candidate ineligible. For a continuation with hard
affinity, transient transport backoff, health, load, or soft quota signals must
not silently select another owner. The continuation either uses its owner,
uses an explicitly approved disaster-recovery member of its pinned fallback
set before commit, or fails with a stable local precommit error.

## Stage 2: Deterministic Integer Scoring

Only eligible candidates enter scoring. The scoring contract is revisioned and
uses checked, saturating fixed-point integer arithmetic. Floating point, random
jitter, hash-map iteration order, and arrival order are prohibited.

~~~text
score =
    priority_component
  + affinity_component
  + trust_and_risk_component
  + health_component
  + latency_component
  + load_component
  + quota_headroom_component
  + cost_component
~~~

Each component has a documented normalized range, weight, missing-data rule,
and clamp behavior in `ScoreRevision`. Cost uses the pricing revision captured
by the request. Health, latency, load, and quota observations carry explicit
freshness; stale or absent soft data receives the revision's deterministic
neutral or penalty value, never a synchronous refresh.

The final ordering is:

1. total score, descending;
2. configured provider priority, descending;
3. `ProviderInstanceId`, ascending;
4. `ProviderRevisionId`, ascending;
5. `DeploymentId`, ascending; and
6. `ProviderModelId`, ascending.

The decision records total score and every component for each retained
candidate, plus the score revision and source snapshot revisions. Candidate and
reason lists are truncated deterministically at configured limits and record
the omitted count. No raw request content, secret, token, full endpoint URL, or
provider response enters the breakdown.

## Health and Circuit State

Circuit state is keyed narrowly enough to avoid poisoning unrelated traffic:

~~~text
tenant + provider instance + provider revision + deployment + endpoint class
~~~

Endpoint classes include at least responses, compact, websocket/live, and other
unary traffic. Transport failure, overload, quota exhaustion, authentication
failure, and policy denial remain separate classifications.

The state machine is `closed`, `open`, and `half_open` with bounded counters,
timestamps, and deterministic backoff. Background workers alone schedule and
execute half-open probes. The request path consumes a snapshot and never waits
for a usage or health probe.

Probe workers have bounded concurrency, queue capacity, per-key deduplication,
timeouts, jittered schedules, and stale-entry eviction. Queue saturation drops
or coalesces refresh work according to policy and emits a bounded metric; it
must not create an unbounded task or state map. Probe success for one endpoint
does not clear unrelated endpoint failures, quota backoff, or revocation.

Existing response streams are not interrupted when a circuit opens. New fresh
requests use the next immutable health snapshot. Hard-affinity continuations
retain their owner subject to the explicit policy and revocation rules above.

## Pricing, Quota, Cost, and Load

Pricing is immutable and versioned by provider revision, model, deployment,
region, unit, and effective interval. All monetary values use integer minor or
micro units with an explicit currency; routing and reconciliation never use
binary floating point.

The execution sequence is:

~~~text
estimate usage and cost under captured pricing revision
-> evaluate tenant, user, project, provider, and credential budgets
-> create idempotent atomic reservation
-> dispatch selected candidate
-> record actual or bounded estimated usage
-> reconcile completed, partial, failed, or cancelled execution
-> release unused reservation exactly once
~~~

Reservation failure is a precommit local constraint. It may remove that
candidate and select the next member of the original eligible set, within the
decision's attempt and time bounds. It must not trigger a broader replan.

Quota classification follows upstream evidence. An explicit provider quota or
rate-limit code may update quota state and permit eligible precommit fallback.
A generic `429` is passed through and must not be reclassified as account quota.
Transport backoff, provider overload, and quota backoff remain separate.

Load is a short-lived bounded signal scoped to provider instance, deployment,
and endpoint. It may influence fresh-request scoring and admission, but it does
not override hard continuation affinity. Counters are acquired and released on
all completion, error, timeout, cancellation, and dropped-stream paths.

## Affinity, Pinning, Fallback, and Commit

A successful initial decision establishes a typed binding containing:

- provider instance, provider revision, deployment, model, and adapter;
- policy, classification, registry, pricing, and score revisions;
- original eligible-set hash and server-side bounded eligible set;
- decision ID, tenant, session or continuation key, and expiry; and
- credential scope reference, never credential material.

Bindings for `previous_response_id`, `x-codex-turn-state`, and session-scoped
routes retain the current affinity guarantees. Client-provided binding hints are
accepted only after tenant, integrity, expiry, and server-side ownership checks.

Before every continuation dispatch, Prodex rechecks only non-negotiable safety
conditions: tenant binding, explicit policy prohibition, provider revocation,
credential authorization, and pinned revision availability. A normal policy or
registry refresh does not silently repin an existing continuation.

Fallback obeys all of the following:

1. It occurs only before the first successful unary response or first streaming
   response commit.
2. Candidates come from the original ordered eligible set.
3. Revalidation may subtract a revoked, newly prohibited, unavailable, or
   reservation-ineligible candidate; it may not add a candidate.
4. Attempts and wall-clock budget are bounded.
5. Each attempt and reason is attached to the same routing decision audit chain.
6. A committed or cancelled request cannot rotate.

Revocation blocks new dispatch immediately after the signed invalidation is
accepted. It does not splice or rotate an already committed stream; the stream
finishes or fails naturally and the outcome is audited. A later continuation
requires an explicitly policy-approved recovery path or fails closed.

## Provider SPI Boundary

The provider SPI translates a governed route into upstream protocol behavior.
It does not choose providers, evaluate policy, rank candidates, manage tenant
budgets, or reinterpret upstream application errors.

The shared contract covers:

- adapter identity and supported wire formats;
- capability negotiation with native, translated, unsupported, and untested
  states;
- canonical request translation and metadata preservation;
- endpoint-specific request construction;
- response and stream normalization without buffering unbounded bodies;
- usage extraction and bounded estimation;
- explicit upstream error classification;
- cancellation and commit signaling; and
- redacted conformance diagnostics.

The application validates `ProviderInvocation`, resolves its `SecretRef` just
in time, supplies the selected authorization, and owns bounded transport
execution. Adapters replace only selected-provider authentication and
transport-local headers. They preserve request `User-Agent`, session and Codex
metadata, upstream status, body, and stream payload where applicable.

The local OpenAI-compatible adapter is a normal adapter kind with the same
registry, capability, routing, audit, and conformance requirements. It is not a
policy bypass. An unsupported capability is rejected before secret resolution
or network dispatch.

## Decision Audit and Observability

Immediately before dispatch, the data plane must prove that:

- the decision belongs to the authenticated tenant and request;
- selected route and credential scope match the decision;
- all required revisions and the original eligible-set hash are present;
- reservation and concurrency admission succeeded; and
- mandatory audit accepted the decision event under the configured failure
  policy.

The audit event includes decision and request IDs, tenant, selected opaque
resource IDs, revisions, eligibility/score reason codes, fallback attempt,
reservation reference, outcome, and timestamps. It excludes prompt/response
content, secrets, tokens, raw credentials, and unrestricted endpoint URLs.

Operational traces may retain a smaller bounded candidate breakdown for live
diagnostics, but they do not replace the durable audit record. Metrics use
bounded labels and cover decision outcome, no-eligible-candidate reasons,
shadow mismatch, circuit state, probe pressure, fallback attempts, reservation
outcome, and reconciliation lag.

## Migration and Cutover

Migration must preserve one authority at every stage:

1. **Registry observe:** compile current provider configuration into read-only
   registry snapshots and compare it with current dispatch configuration.
2. **Shadow:** run the pure planner after current admission, emit redacted
   mismatch telemetry, and leave legacy selection fully authoritative.
3. **Canary hard filter:** for explicitly enrolled tenants, enforce governed
   eligibility while retaining the legacy ordering inside the eligible set.
4. **Canary score:** enable deterministic governed scoring for enrolled tenants;
   retain existing transport, affinity, retry, and no-midstream code.
5. **Cutover:** make the application routing use case the sole producer of
   production `ProviderInvocation`; reject dispatch without a valid decision.
6. **Removal:** delete legacy provider/model ranking authority and compatibility
   bypasses after rollback criteria, audit evidence, and soak thresholds pass.

A rollback changes an entire tenant or deployment cohort to the previous
authoritative mode. It must not allow the legacy and governed planners to make
independent dispatch decisions for the same request. Provider revocation and
mandatory compliance filters remain enforced during rollback.

Promotion gates include zero unexplained shadow compliance mismatches, bounded
planner latency at maximum configured cardinality, audit completeness,
reservation/reconciliation correctness, no affinity regressions, no probe
storms, and successful provider conformance runs.

## Verification Matrix

### Registry and Isolation

- Draft, validation, maker-checker approval, activation, suspension, revocation,
  retirement, stale precondition, idempotency, and last-known-good behavior.
- Cross-tenant ID, alias, credential, cache, revision, and audit isolation.
- Invalid checksum, missing cross-reference, excessive cardinality, expired
  revision, activation race, invalidation loss, and restart recovery.
- Secret serialization, debug, log, metric, trace, error, and audit redaction.

### Eligibility and Scoring

- Matrix tests across classification, residency, retention, training use, trust,
  risk, model, endpoint, context, capability, status, circuit, quota, and budget.
- Property tests prove that ineligible candidates are never selected, input
  permutation does not change output, ties are stable, integer arithmetic does
  not overflow, and truncation preserves the selected candidate.
- Golden tests pin score revisions, component values, reason codes, fallback-set
  hashes, and redacted decision serialization.

### Affinity and Failure

- Previous-response, turn-state, session, compact, websocket, and unary affinity.
- Pinned policy/registry/pricing revisions across refresh, restart, and replicas.
- Suspension versus revocation, explicit policy prohibition, unavailable pinned
  revisions, approved disaster recovery, and invalid client hints.
- Connect failure, timeout, overload, explicit quota, generic `429`, partial
  stream, cancellation, dropped body, first-byte commit, and retry exhaustion.
- Assertions that no fallback occurs after commit and no fallback candidate is
  outside the original eligible set.

### Health, Accounting, and Adapters

- Endpoint circuit isolation, transitions, deterministic backoff, stale
  snapshots, bounded half-open probes, queue saturation, deduplication, and
  multi-replica probe storms.
- Reservation idempotency, concurrent budget exhaustion, cancellation, partial
  usage, missing usage, pricing revision rollover, reconciliation retry, and
  exact release of unused funds.
- One shared adapter suite for request metadata, capability rejection,
  translation, status/body pass-through, SSE/websocket boundaries, usage,
  cancellation, malformed upstream data, secret redaction, and local
  OpenAI-compatible behavior.

### Benchmarks and Load

Benchmarks cover the configured maximum provider, deployment, model, alias,
reason, and fallback cardinalities. They measure registry compile/swap, hard
filter, score, trace construction, affinity lookup, circuit snapshot lookup,
reservation contention, and fallback planning. Results report p50, p95, p99,
allocation, and throughput under concurrent tenants and refreshes.

Existing runtime hot-path benchmarks with 64 to 96 profile candidates remain
regression coverage, but they do not substitute for governed registry,
eligibility, scoring, refresh, and reconciliation benchmarks.

## Current Repository Baseline and Gaps

| Area | Existing evidence | Gap to this design |
| --- | --- | --- |
| Provider identity and catalog | `prodex-provider-core` has a fixed `ProviderId`, static adapters/translators, and an embedded model catalog | No tenant provider-instance identity, immutable registry revision, approval, activation, suspension, or revocation |
| Provider invocation | `prodex-provider-spi` carries tenant, principal, route, `SecretRef`, stream mode, and usage estimate | Invocation is built from an already configured provider rather than an authoritative routing decision |
| Capability contract | Provider core and SPI expose explicit capability status and translation conformance | Capability negotiation is not the production provider-selection authority |
| Routing | Runtime selection preserves profile affinity and ranks quota, health, inflight load, priority, and cache signals; gateway aliases support cost/latency strategies | These selectors do not hard-filter classification, residency, retention, trust, risk, and governance status before scoring |
| Decision explanation | Runtime route traces are bounded and redacted | They are diagnostic and non-durable; they lack policy, registry, pricing, and score revisions and component proofs |
| Health | Endpoint-aware transport backoff, circuit state, half-open reservation, and a background refresh queue exist | Selection can still perform synchronous usage probes; governance snapshots and strict queue/state bounds are incomplete |
| Retry and affinity | Precommit retry, hard profile affinity, and no-retry-after-first-byte/cancellation are used in production | Fallback is not pinned to an original governed eligible set, and registry/policy revocation behavior is undefined |
| Cost and quota | Static microusd model costs, estimates, atomic reservations, and reconciliation primitives exist | Governed pricing revisions and complete tenant/user/project/provider decision inputs are absent; some spend events use floating point |
| Adapter execution | Static adapter/translator contracts and provider fixture suites exist | Actual network dispatch remains provider-specific application code rather than one production SPI dispatch contract |
| Production gate | The local rewrite pipeline performs admission, constraints, governance hooks, accounting, and dispatch | Provider identity is derived from the configured bridge before dispatch; no durable governed `RoutingDecision` is required |

Relevant current documentation includes
[`provider-conformance.md`](../provider-conformance.md),
[`provider-capabilities.md`](../provider-capabilities.md), and
[`runtime-policy.md`](../runtime-policy.md). The phased ownership and canonical
pipeline are defined in
[`01-target-architecture.md`](01-target-architecture.md), while implementation
status remains tracked in
[`implementation-ledger.md`](implementation-ledger.md).

Phase 4 is complete only when every production provider dispatch can be joined
to one durable decision proving tenant binding, hard eligibility, deterministic
selection, pinned revisions, reservation, bounded fallback, and commit state.
