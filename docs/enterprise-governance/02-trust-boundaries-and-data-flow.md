# Trust Boundaries and Data Flow

## Boundary Rules

1. External requests, identity evidence, provider responses, detector output,
   configuration publications, storage rows, and secret-provider responses are
   untrusted until their owning adapter validates them.
2. Pure policy and routing decisions consume immutable bounded values only.
3. Raw request and model content never enters logs, metrics, traces, audit,
   health, caches, approval metadata, or ordinary governance storage.
4. PostgreSQL is authoritative for enterprise durable state. Redis is
   rebuildable coordination only. SQLite remains local-mode storage.
5. Remote SIEM export is asynchronous through a durable outbox. Secret values
   resolve immediately before adapter use and do not enter registry records.

## Data-Plane Sequence

~~~mermaid
sequenceDiagram
    participant C as Channel
    participant G as Gateway
    participant A as Auth and Session
    participant I as Inspection
    participant P as PDP
    participant R as Routing Planner
    participant S as Provider SPI
    participant U as Provider
    participant D as Durable Store
    participant O as Audit Outbox

    C->>G: bounded canonical request
    G->>A: credential evidence and route
    A-->>G: tenant, principal, scope, session
    G->>I: supported bounded content fields
    I-->>G: coverage, findings, classification, risk
    G->>P: immutable PolicyInput
    P-->>G: effect, obligations, reasons, revision
    G->>G: mask and enforce admission obligations
    G->>R: obligations, registry, health, quota, affinity
    R-->>G: eligible set, score, fallback set, revisions
    G->>D: atomic usage reservation
    G->>S: validated invocation plus SecretRef
    S->>U: provider request
    U-->>S: unary or stream response
    S-->>G: normalized response events
    G->>G: incremental response guard
    G->>D: reconcile completion or cancellation
    G->>O: mandatory redacted decision events
    G-->>C: stable response or natural transport failure
~~~

No provider retry or rotation occurs after first response commit. Continuation
affinity is authoritative when still permitted by the pinned policy and active
registry.

## Control-Plane Publication

~~~mermaid
sequenceDiagram
    participant M as Maker
    participant CP as Control Plane
    participant DB as Governance Store
    participant K as Checker
    participant B as Audit and Outbox
    participant G as Gateway Replicas

    M->>CP: create draft with idempotency and precondition
    CP->>CP: parse, validate, compile, analyze, fingerprint
    CP->>DB: persist immutable revision
    CP->>B: append draft audit event
    M->>CP: submit exact fingerprint
    CP->>DB: PendingApproval
    K->>CP: approve exact tenant, scope, and fingerprint
    CP->>CP: reject self-approval, replay, expiry, or insufficient quorum
    CP->>DB: atomic approval and activation
    CP->>B: append activation event and invalidation outbox
    B-->>G: revision invalidation
    G->>DB: bounded background load
    G->>G: verify then atomically swap snapshot
    G-->>B: acknowledgement
~~~

Load or verification failure preserves the prior last-known-good revision.
Urgent revocation uses an explicit fail-closed invalidation path.

## Identity and Session Flow

Human tokens and service credentials cross the authentication boundary only
after exact issuer, audience, tenant, credential scope, authentication
strength, signature, expiry, and revocation checks. Discovery and JWKS refresh
run outside request handling and publish an immutable verified cache snapshot.

Sessions bind to tenant, principal, channel policy, credential scope, absolute
and idle expiry, revocation state, classification ceiling, policy revision, and
provider affinity. Optional network risk uses only trusted-proxy input and
stores a coarse or keyed representation, never a full IP address.

## Inspection Boundary

The inspection application use case accepts only supported content locations
from a bounded schema walker. Local detectors and the Presidio adapter return
finding metadata without matching values. Unsupported binary, image, audio, or
unknown fields produce Partial or Unsupported coverage; they are never labeled
Full without a real adapter.

The current baseline violates the final target in two known places:

- Presidio redaction and runtime guardrail PII redaction are separate policy
  paths.
- Gemini Live WebSocket dispatch can occur before HTTP body capture and the
  common inspection stage.

Phase 1 removes both gaps while preserving personal-mode compatibility.

## Provider and Secret Flow

~~~mermaid
flowchart LR
    REG[Approved registry metadata] --> FILTER[Hard eligibility]
    POLICY[Policy obligations] --> FILTER
    HEALTH[Bounded dynamic snapshot] --> FILTER
    FILTER --> SCORE[Fixed-point score]
    SCORE --> DECISION[Routing decision]
    DECISION --> SPI[Provider SPI]
    REF[SecretRef] --> VAULT[External secret provider]
    VAULT --> MATERIAL[Scoped secret material]
    MATERIAL --> SPI
    SPI --> UPSTREAM[Provider]
~~~

Provider credentials are absent from registry, route decision, audit, debug,
health, API, and metrics values. The adapter resolves a SecretRef close to use,
holds material for the shortest practical lifetime, and uses bounded renewal
and safe expiry.

## Audit and SIEM Flow

Mandatory events are built from typed redacted metadata, chained per tenant or
partition, and committed transactionally with high-risk mutations. A durable
outbox exports events at least once with stable event IDs, bounded batches,
retry/backoff, and dead-letter handling. A remote SIEM outage cannot block the
data path, but inability to durably record mandatory local audit data fails the
affected bank-mode operation according to the failure matrix.

## Failure Ownership

| Failure | Owner | Required behavior |
| --- | --- | --- |
| Invalid route, identity, session, or body | Gateway/auth boundary | Stable redacted denial |
| Required inspection unavailable | Governance application | Mode-specific deny; bank fails closed |
| Invalid policy or registry publication | Control plane | Reject candidate; retain LKG |
| No eligible provider | Routing planner/application | Local 503 before upstream response |
| Upstream response exists | Provider/transport | Preserve upstream status/body/stream semantics |
| Pre-commit temporary provider failure | Routing/application | Bounded retry inside original eligible set |
| Post-commit transport or guard failure | Response orchestration | Terminate naturally/safely; reconcile; audit |
| Durable mandatory audit unavailable | Application/storage | Fail protected bank operation closed |
| SIEM unavailable | Exporter | Queue durable outbox; alert on lag/dead letters |
| Vault lease expires | Secret adapter | Stop using material; bounded refresh; fail safe |
