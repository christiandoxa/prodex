# Policy Authority and Revision Store

## Status and Scope

This document defines the contract for policy administration, approval,
publication, evaluation, activation, rollback, and last-known-good operation.
The repository implements the durable SQLite/PostgreSQL lifecycle, HTTP admin
surface, maker-checker approvals, activation, rollback, snapshots, and audit
outbox, plus a bounded typed compiler/PDP and detached Ed25519 artifact
verification. Deployment-specific operational evidence remains environment
acceptance. HTTP/OpenAPI is the canonical administration surface; a duplicate
CLI lifecycle is not part of the current product contract.

The boundary uses Prodex domain, control-plane, configuration, and
storage boundaries. It does not turn `policy.toml`, a provider adapter, a route
handler, Redis, or an external policy service into a second policy authority.
The pure policy evaluator and routing planner perform no filesystem, database,
network, Vault, SIEM, clock, or environment access.

The terms **must**, **must not**, **required**, **should**, and **may** are
normative in this document.

## Current Source Boundary

| Area | Implemented control | Deliberate boundary or environment acceptance |
| --- | --- | --- |
| Snapshot integrity | Immutable revisions, SHA-256 digests, detached Ed25519 signatures, activation state, active/LKG pointers, approval quorum, and stable repository errors; tenant/kind/revision/checksum binding is reverified on every lifecycle load. | Private signing-key custody and rotation ceremony are deployment responsibilities. |
| Configuration publication | `prodex-application` provides policy governance services over tenant-bound revision, approval, activation, rollback, revoke, and snapshot repository operations. | Runtime operational configuration remains separate from governance policy authority. |
| Runtime policy | `prodex-runtime-policy` parses, validates, caches, reloads, and invalidates the local runtime policy file. | The runtime file remains operational configuration. It is not the revisioned tenant policy store or the sole PDP source. |
| Runtime governance invalidation | PostgreSQL pointer changes transactionally append a tenant/kind outbox event and emit a bounded `NOTIFY` wake-up. Each gateway replica replays unacknowledged events at startup and on a bounded poll, refreshes the authoritative snapshot before acknowledging, and compacts only events acknowledged by every live replica while retaining the latest head per artifact kind. | `NOTIFY` remains only a latency hint. PostgreSQL availability, replica-lease expiry, and deployment convergence alerts remain operational responsibilities. |
| Control-plane authorization | `prodex-control-plane` and the application layer enforce tenant, role, credential scope, idempotency, maker-checker separation, and append-only audit planning for lifecycle mutations. | Deployment-specific role and identity mappings remain operator-owned. |
| Break glass | Durable approval records enforce reason, scope, expiry, quorum, maker-checker separation, revocation, and bounded consumption through the governance repositories. | Environment-owned review and incident procedures remain deployment responsibilities. |
| Durable storage | SQLite and PostgreSQL repositories implement revisions, approvals, votes, activation history, active/LKG snapshots, execution approvals, audit chaining, and SIEM outbox state. | File and Redis are intentionally unsupported governance authorities; production PostgreSQL HA remains deployment evidence. |
| Administration API | Authenticated gateway HTTP routes and checked OpenAPI expose revision, validation, submission, voting, activation, rollback, revoke, status, and inspection operations. | HTTP/OpenAPI is canonical; a duplicate first-class CLI is intentionally out of scope. |

These source controls must not be described as deployment evidence.

## Policy Authority Boundaries

The implementation separates four responsibilities:

| Role | Responsibility | Prohibited responsibility |
| --- | --- | --- |
| Policy Administration Point (PAP) | Authenticated CLI/API operations for validation, drafts, submission, approval, activation, rollback, status, and history. | It must not evaluate live prompt content or write around approval/storage contracts. |
| Policy Information Point (PIP) | Supplies bounded, typed principal, session, classification, coverage, environment, quota, provider, and capability attributes captured for one request. | It must not fetch missing attributes synchronously from the PDP or invent a permissive default. |
| Policy Decision Point (PDP) | Pure, deterministic evaluation of one immutable compiled policy snapshot and one `PolicyInput`. | It performs no I/O, mutation, secret resolution, external scripting, or arbitrary regex compilation. |
| Policy Enforcement Points (PEPs) | Enforce the returned effect and obligations at gateway admission, transformation, reservation, routing, provider dispatch, response inspection, and control-plane mutation. | A PEP must not reinterpret source policy or silently discard an obligation. |

Every governed request captures exactly one verified policy revision before
evaluation. The same revision and decision identifier follow the request to
routing, provider dispatch, response handling, accounting, and audit. Session
records persist revision pins as admission guards. If a later request no longer
matches the currently published compatible policy and provider revisions, the
continuation fails closed and must start a new governed session; the request
path does not query or resurrect historical revisions.

## PDP Contract

The policy source compiles into a bounded, schema-versioned artifact. The data
plane uses only that artifact, never policy source text.

~~~text
PolicyInput
  tenant and pseudonymous principal reference
  principal kind, roles, groups, and credential scopes
  channel, route, action, and resource kind
  authentication strength, MFA, network zone, and trusted-proxy facts
  session age, risk, revocation state, and pinned revisions
  data classification, inspection coverage, and bounded risk signals
  requested model, capabilities, tools, modalities, and token/context limits
  quota, budget, and execution-approval facts
  bounded provider trust, region, and data-handling facts when relevant
  explicit evaluation time and request deadline

PolicyDecision
  decision_id
  effect: allow | deny
  stable bounded reason codes
  request obligations
  response obligations
  routing eligibility obligations
  audit and retention obligations
  execution-approval requirement
  policy revision, artifact fingerprint, and evaluator version
~~~

Explicit deny wins. Conflicting obligations resolve to the more restrictive
result or to a stable denial when no safe merge exists. Missing required
attributes fail closed in `enterprise_enforce` and `bank_enforce`. In observe
mode, an incomplete shadow decision is recorded as such and cannot be reported
as an allow decision.

The compiler rejects unknown fields, duplicate or unreachable rules,
contradictory obligations, cycles, unbounded collections, excessive nesting,
excessive rule or attribute counts, unsupported operators, unsafe patterns,
integer overflow, and an artifact above its configured byte limit. Evaluation
has bounded steps, collection cardinality, output reasons, and obligations.

## Revision and Draft Lifecycle

Policy source is editable only while it is a draft. A submitted candidate is
canonicalized and fingerprinted into a new immutable revision.

~~~text
draft
  -> validating -> ready_for_submission -> pending_approval
  -> rejected | expired | cancelled
pending_approval -> approved -> active -> superseded
approved -> expired | cancelled
active -> rolled_back_from | invalidated
~~~

`rejected`, `expired`, and `cancelled` are terminal for the submitted revision.
Editing any policy field, approval scope, required quorum, compiler version, or
dependency reference creates a new fingerprint and requires a new submission.
An immutable revision is never patched in place.

Publication is equivalent to:

~~~text
parse
-> validate schema and bounded values
-> compile
-> static analysis
-> canonical fingerprint and mandatory detached signature verification in enforcement modes
-> persist immutable revision
-> submit and collect approval
-> atomically activate with mandatory audit, SIEM outbox, and invalidation outbox event
-> gateways load, verify, and atomically swap
-> promote only a verified, successfully loaded revision to LKG
~~~

Validation and compilation happen before approval so reviewers approve the
exact canonical source, artifact fingerprint, compiler version, dependencies,
and impact summary that could be activated. A successful probe or validation
does not activate a revision.

## Maker-Checker Approval

### Approval request

An approval request is tenant- and resource-scoped and contains no raw prompt,
response, secret, token, provider credential, or arbitrary request header. It
contains at least:

- immutable revision and artifact fingerprints;
- maker principal reference and authentication strength;
- operation and scope, such as activate, roll back, declassify, change provider
  trust, change mandatory audit, or grant break glass;
- required reviewer roles and quorum;
- created, not-before, and expiry times;
- a bounded machine-generated impact summary and stable risk codes;
- idempotency key, request fingerprint, ETag/version, and status;
- reviewer votes, decisions, reason codes, and authentication context; and
- links to activation, rollback, break-glass, and audit event identifiers.

### State-machine invariants

1. The maker cannot approve their own request, even through another role,
   credential, group, service identity, or replayed token.
2. Reviewer identities are unique for quorum. Multiple credentials or votes
   from one principal count once.
3. Each reviewer must be authorized for the same tenant, resource, operation,
   scope, and current approval-policy revision at vote time.
4. A vote is bound to the immutable candidate fingerprint and request version.
   It cannot transfer to a changed candidate.
5. Rejection, cancellation, or expiry prevents activation. A later approval
   cannot revive a terminal request.
6. Quorum and separation rules are evaluated transactionally. Concurrent votes
   cannot double-count or skip a terminal transition.
7. Idempotent replay returns the original result only when tenant, operation,
   request fingerprint, and idempotency key all match. A collision is a stable
   conflict.
8. Activation requires an optimistic `If-Match`/ETag precondition against the
   current active pointer and the approval request version.
9. Approval does not imply indefinite activation authority. Approval and
   activation windows are bounded.
10. Every successful and failed submit, vote, activation, cancellation,
    expiry, and rollback attempt is audited.

The default high-risk policy is two-person control: one maker and at least one
independent checker. Tenant policy may require a larger quorum or specialist
review roles, but cannot reduce the `bank_enforce` minimum through an ordinary
feature flag.

### Break glass

Break glass is a narrow, expiring grant, not a global bypass. The durable grant
must bind tenant, principal, operation, resource, reason code, start and expiry,
maximum uses, required authentication strength, issuer, independent checker,
policy revision, and revocation status. Maximum duration and scope are bounded
at publication. Each use is consumed atomically and audited; exhaustion,
expiry, revocation, missing audit, or scope mismatch fails closed.

Break glass cannot authorize raw secret configuration, cross-tenant access,
audit suppression, unsupported classification, an unapproved provider, or a
mid-stream provider change. Post-use review is mandatory and has an alerting
deadline. The existing in-memory authorization shape is retained only as a
validated input to this durable decision, not as proof that a grant exists.

### Optional execution approval

Execution approval is disabled by default. If enabled, it stores a keyed
request fingerprint, classification, stable risk and obligation codes, policy
revision, permitted action/capability scope, expiry, and one-time consumption
key. It does not store prompt, response, tool arguments, detector matches, or
credentials. Consumption and execution reservation are atomic or use a
recoverable exactly-once protocol; an expired or replayed approval cannot
dispatch a provider request.

## Authoritative Store

PostgreSQL is authoritative in enterprise multi-replica modes. SQLite provides
local, single-node compatibility and migration tests. Redis may carry bounded
invalidation notifications or leases; deleting Redis must not change policy,
approval, activation, or LKG state.

The minimum conceptual schema is:

| Entity | Required invariant |
| --- | --- |
| `policy_drafts` | Tenant-scoped mutable draft with bounded source, version/ETag, author, timestamps, and no secret material. |
| `policy_revisions` | Immutable canonical source, schema/compiler versions, fingerprint, validation result, creator, and creation time. Unique by tenant and fingerprint. |
| `policy_artifacts` | Immutable bounded compiled artifact or deterministic compilation metadata and artifact digest. |
| `policy_dependencies` | Tenant-bound revision references for classification, approval policy, provider registry, and other compile-time dependencies. |
| `policy_approval_requests` | Candidate fingerprint, maker, operation, quorum, expiry, status, version, and idempotency link. |
| `policy_approval_votes` | One vote per tenant/request/reviewer with outcome, reason code, authentication context, and timestamp. |
| `tenant_policy_pointers` | One active and one LKG pointer per tenant, with pointer version and activation epoch. |
| `policy_activation_history` | Append-only activation, rollback, invalidation, promotion, and failure records. |
| `execution_approvals` | Optional metadata-only request authorization with expiry and one-time consumption state. |
| `break_glass_grants` and `uses` | Approved bounded grant plus append-only atomic use records and review status. |
| `control_idempotency` | Tenant, operation, key, request fingerprint, state, and bounded stored result metadata. |
| `governance_invalidation_outbox` | Durable tenant/kind cache-fanout events with bounded per-replica leases and acknowledgements. It accelerates convergence but is never the policy authority. |

Every tenant-owned table has a tenant key, tenant-scoped primary/unique and
foreign keys, bounded text/binary lengths, explicit status checks, and
PostgreSQL RLS with transaction-local tenant context. Gateway identities are
`NOSUPERUSER NOBYPASSRLS`, do not own tables, and cannot run DDL.

### Activation transaction

Activation is one authoritative transaction:

1. Lock the tenant policy pointer or compare its version.
2. Re-read the immutable candidate, dependencies, approval request, unexpired
   quorum, and revocation state inside the transaction.
3. Reject stale ETags, stale approval-policy revisions, changed dependencies,
   self-approval, and terminal approval state.
4. Append activation history and the mandatory audit chain event.
5. Advance the active pointer while retaining the previous proven LKG pointer.
6. Insert the durable SIEM outbox record and update the pointer. On PostgreSQL,
   the pointer trigger appends a durable tenant/kind invalidation event in the
   same transaction and queues a bounded `NOTIFY` wake-up for delivery after
   commit.
7. Commit all records or none.

An ambiguous commit result is resolved by idempotency key and request
fingerprint before retry. No retry creates a second activation or approval use.

## Last-Known-Good Runtime Contract

Gateways refresh policy snapshots in bounded background work. A request path
does not query the policy store. PostgreSQL replicas register an expiring
replica identity, replay unacknowledged invalidation events at startup and on a
bounded poll, and use transactional pointer-change notifications only to wake
that replay early. An event is acknowledged only after the same authoritative
refresh succeeds. Safe compaction removes events acknowledged by every live
replica while retaining the latest event for each artifact kind so a restarted
replica can rebuild from the current head.

1. A refresh reads the active revision and all pinned dependencies.
2. It verifies tenant binding, schema and compiler compatibility, digest,
   the configured Ed25519 signature in enforcement modes, bounded artifact shape, and dependency
   fingerprints.
3. It constructs evaluator indexes off the request path.
4. It atomically swaps one immutable snapshot only after all verification
   succeeds.
5. It reports the loaded revision. The control plane promotes a revision to
   tenant LKG only after the configured healthy gateway acknowledgement rule is
   met.
6. A malformed or unsupported candidate leaves the current LKG loaded and
   emits a stable failure audit and alert.

Snapshots have explicit refresh, stale, and expiry windows. Before expiry, an
uninvalidated LKG may serve according to mode and policy. A security
invalidation takes precedence over LKG. `bank_enforce` fails closed when no
valid, non-expired, non-invalidated policy snapshot exists. Rollback activates
a prior immutable revision through the same approval, audit, outbox, verify,
and acknowledgement sequence; it does not rewrite history.

## Failure Matrix

| Failure | `personal` | `enterprise_observe` | `enterprise_enforce` | `bank_enforce` |
| --- | --- | --- | --- | --- |
| Shadow PDP unavailable or candidate invalid | Preserve documented legacy behavior; emit bounded local diagnostic. | Legacy remains authoritative; mark shadow decision unavailable and alert. | Do not activate candidate; continue with valid unexpired LKG. If none exists, deny governed requests. | Same, and readiness fails when mandatory policy cannot be served. |
| Policy store unavailable during request | Use local configured behavior. | Use captured shadow snapshot; no synchronous store call. | Use valid captured active/LKG snapshot; deny after policy expiry. | Same fail-closed rule; never query storage from the PDP. |
| Policy store unavailable during draft/read | Return stable service-unavailable response; no state change. | Same. | Same. | Same. |
| Approval service or quorum unavailable | No high-risk activation. | No high-risk activation. | Fail the mutation closed. | Fail the mutation closed and alert. |
| Candidate parse, compile, fingerprint, signature, or dependency verification fails | Reject candidate. | Reject candidate. | Reject candidate; retain LKG. | Reject candidate; retain LKG and alert on attempted activation. |
| Mandatory audit or transactional outbox insert fails during mutation | Roll back mutation where the governed store is used. | Roll back mutation. | Roll back mutation. | Roll back mutation; no approval, activation, rollback, or break-glass use is acknowledged. |
| Pointer wake-up is delayed or missed after a committed lifecycle mutation | The authoritative poll repairs the cache; local behavior is otherwise unchanged. | Continue the prior shadow snapshot only until bounded outbox replay refreshes it. | The handling replica publishes before acknowledging the mutation; every live replica replays the durable event, refreshes before acknowledging, and retains failed events for retry. Existing requests retain their captured snapshot. | Same, while a refresh error keeps the event unacknowledged, invalidates the signaled cache entry, and affects readiness. |
| Database commit result is unknown | Resolve by tenant/idempotency key before retry. | Same. | Same; never issue a second activation or consume approval twice. | Same, with operator alert after bounded resolution attempts. |
| Active revision explicitly invalidated | Ignore in disabled mode. | Stop observing with that revision when the handling path or bounded refresh observes the commit. | Stop new use immediately on the handling or replaying replica; with reachable healthy PostgreSQL, every live replica converges through bounded outbox replay. Use a distinct valid LKG only if invalidation scope permits. | Same convergence contract; keep failed events pending, invalidate the signaled local cache, and fail readiness when authoritative refresh is unavailable or no permitted revision exists. |
| Break-glass grant store, audit, expiry, or use counter unavailable | Deny break-glass use. | Deny break-glass use. | Deny break-glass use. | Deny break-glass use and alert. |
| Execution approval cannot be atomically consumed | Do not dispatch the approved execution. | Do not treat approval as satisfied. | Deny dispatch. | Deny dispatch and alert on replay or ambiguity. |

Stable external errors reveal no policy source, rule index, tenant existence,
reviewer identity, or another tenant's state. Richer internal reasons remain
bounded and redacted in audit.

Revoke must remain disabled during a mixed-version rollout until every serving
replica runs the outbox replay listener and authoritative refresh contract.
Schema terminality prevents an older writer from reviving a revoked revision,
but it cannot invalidate an older process's already-loaded cache. Old replicas
must therefore be drained before operators enable revoke.

## API, CLI, and Control-Plane Surface

The target API follows existing canonical mounts and checked OpenAPI practice.
Equivalent endpoints are required for:

- validate a candidate without persisting it;
- create, update, list, and show drafts;
- submit and inspect immutable revisions;
- list approval requests and approve, reject, cancel, or inspect them;
- activate, roll back, invalidate, and inspect active/LKG status and history;
- create, revoke, use-status, and review break-glass grants; and
- list, approve, reject, and inspect optional execution approvals when enabled.

Mutations require an authenticated control-plane principal, tenant/resource
authorization, operation-specific scope, `Idempotency-Key`, and `If-Match` for
mutable resources or pointers. Approval and activation use distinct scopes;
ordinary policy authors cannot grant themselves checker or break-glass scope.
All schemas, stable errors, authorization requirements, idempotency behavior,
ETag semantics, and rate limits are represented in the checked OpenAPI
document and contract tests.

The CLI should provide equivalent commands under one policy namespace, for
example `policy validate`, `draft create/list/show/update`, `submit`,
`approval list/approve/reject`, `activate`, `rollback`, `status`, and `history`.
Machine-readable output is versioned and never prints policy secrets or raw
request content. Interactive confirmation is usability protection, not an
authorization or approval control.

## Migration and Cutover

1. Add expand-only PostgreSQL and SQLite migrations plus storage contracts and
   adapter execution tests. Do not execute DDL in gateway startup or request
   paths.
2. Import current operational policy/configuration as a synthetic, immutable
   baseline revision with provenance. Import does not fabricate historical
   approval.
3. Deploy the compiler, store, approval service, and snapshot refresh in
   read-only/shadow mode. Compare deterministic PDP decisions with legacy
   behavior using content-free identifiers and bounded reason codes.
4. Enable tenant-scoped approval and publication while legacy enforcement is
   still authoritative. Exercise invalid candidate, race, rollback, and LKG
   recovery.
5. Move PEPs to enforce the PDP decision and obligations for canary tenants.
   At any instant, one source is authoritative; shadow output never dispatches.
6. Expand enforcement only after refresh lag, decision mismatch, audit, and
   approval evidence meet the gate. Rollback swaps to the verified LKG.
7. Remove legacy policy interpretations and direct runtime-file policy
   authority only after all supported channels and PEPs use the same snapshot.

Contract migrations later remove obsolete columns or paths only after a full
compatibility window, export/rollback plan, and proof that no active binary
depends on them. Approval and activation history is not deleted by a generic
rollback.

## Verification and Exit Gate

Required evidence includes:

- golden PDP decisions, explicit-deny precedence, missing attributes, and
  obligation-conflict tests;
- parser/compiler complexity, boundedness, deterministic fingerprint, and
  evaluator latency tests;
- maker-checker self-approval, shared-principal, quorum, expiry, cancellation,
  replay, cross-tenant, stale-ETag, concurrent vote, and idempotency tests;
- break-glass scope, maximum-use, expiry, revocation, independent approval,
  post-use review, and mandatory-audit tests;
- optional execution-approval expiry, replay, metadata leakage, and atomic
  consumption tests if that capability is enabled;
- PostgreSQL RLS, SQLite compatibility, forward/backward migration, unknown
  commit, concurrent activation, invalidation, and multi-replica refresh tests;
- LKG load failure, corrupt artifact, stale/expired snapshot, security
  invalidation, gateway acknowledgement, and rollback tests; and
- OpenAPI, CLI JSON, scope, error, idempotency, precondition, and no-content
  leakage contract tests.

Phase 3 policy work exits only when the typed PDP and revisioned store are the
sole production policy source, every PEP consumes the same typed decision,
high-risk changes require tested maker-checker approval, and malformed or
unavailable candidates cannot evict a verified LKG.
