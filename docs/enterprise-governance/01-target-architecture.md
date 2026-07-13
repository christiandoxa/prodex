# Enterprise Governance Target Architecture

## Design Intent

Prodex evolves its existing gateway, application, domain, provider, storage,
runtime, and observability boundaries. The six logical layers below do not
require six new crates. Pure decisions remain in domain/provider-core
boundaries; application use cases orchestrate them; adapters perform I/O; and
prodex-app remains composition and compatibility glue.

No statement in this document is a certification or legal conclusion.

## Candidate Implementation State

The release candidate based on baseline `e308fdf6` implements the canonical
typed inspection, classification, policy, obligation, session and governed
routing stages in the local rewrite application boundary. Production guards
require supported HTTP/compact/WebSocket dispatch to consume that boundary and
an audited governed routing decision in enforcement modes.

The implementation is intentionally narrower than the target diagram in four
places:

- one local rewrite process has one attached executable provider adapter;
  simultaneous heterogeneous adapter selection and cross-provider fallback are
  unavailable;
- mandatory governed data-plane audit is a synchronous precommit append;
- policy-selected Presidio and guardrail-webhook calls execute on the request
  path under fixed timeout, redirect, response-size and concurrency bounds; and
- live PostgreSQL governance/RLS and SIEM outbox validation passed; managed
  failover and external SIEM delivery remain deployment acceptance work.

These are declared release residuals, not hidden fallback behavior. The router
must reject a selected provider that is not the attached adapter, never send it
through another adapter, and never retry after response commitment.

## Six-Layer Architecture

~~~mermaid
flowchart TB
    subgraph L1[1. User and Channel]
        CLI[CLI]
        IDE[IDE]
        API[Internal API]
        MCP[MCP or supported machine channel]
        ADMIN[Administrative surface]
    end

    subgraph L2[2. Governance and Policy]
        ID[OIDC and service identity]
        AZ[RBAC and ABAC]
        INSPECT[Inspection and classification]
        PDP[Deterministic PDP]
        APPROVAL[Approval and break-glass]
        AUDIT[Immutable audit and SIEM outbox]
    end

    subgraph L3[3. Prodex Core]
        GW[Canonical gateway boundary]
        APP[Application orchestration]
        SESSION[Session and context]
        RESPONSE[Response orchestration]
        SUPPORT[Errors, config, telemetry, feature state]
    end

    subgraph L4[4. Provider Abstraction and Governed Routing]
        REGISTRY[Revisioned provider registry]
        FILTER[Hard compliance eligibility]
        SCORE[Deterministic fixed-point scoring]
        HEALTH[Health, circuit, load, quota, cost]
        SPI[Provider SPI]
    end

    subgraph L5[5. AI Provider Execution]
        LOCAL[Approved local or on-prem]
        OPENAI[OpenAI and compatible]
        ANTHROPIC[Anthropic]
        GEMINI[Gemini]
        OPTIONAL[Capability-gated optional adapters]
    end

    subgraph L6[6. Data and Infrastructure]
        PG[(PostgreSQL with RLS)]
        SQLITE[(SQLite local mode)]
        REDIS[(Redis ephemeral coordination)]
        VAULT[External secret provider]
        SIEM[SIEM sink]
        OBS[Monitoring and alerts]
        HA[HA deployment, backup, restore, DR]
    end

    CLI --> GW
    IDE --> GW
    API --> GW
    MCP --> GW
    ADMIN --> GW
    GW --> ID --> AZ --> INSPECT --> PDP --> APP
    APP --> SESSION
    APP --> FILTER
    REGISTRY --> FILTER
    HEALTH --> FILTER
    FILTER --> SCORE --> SPI
    SPI --> LOCAL
    SPI --> OPENAI
    SPI --> ANTHROPIC
    SPI --> GEMINI
    SPI --> OPTIONAL
    SPI --> RESPONSE --> APP
    APP --> AUDIT
    APPROVAL --> PDP
    APPROVAL --> REGISTRY
    PDP --> PG
    REGISTRY --> PG
    SESSION --> PG
    APP --> SQLITE
    APP --> REDIS
    SPI --> VAULT
    AUDIT --> PG
    AUDIT --> SIEM
    SUPPORT --> OBS
    PG --> HA
~~~

## Canonical Data-Plane Pipeline

One immutable request context carries redacted metadata and typed decisions:

~~~text
canonical request and headers
-> request ID, trace, deadline, tenant, channel, credential scope
-> authentication and session validation
-> route/action authorization
-> bounded typed request envelope
-> inspection coverage, findings, classification, risk
-> immutable policy snapshot evaluation
-> mandatory request obligations and masking
-> rate, quota, concurrency, and execution approval
-> hard provider eligibility
-> deterministic provider scoring and reservation
-> provider SPI dispatch
-> response normalization and incremental guard
-> accounting reconciliation
-> mandatory audit and bounded telemetry
-> stable redacted response
~~~

Raw content stays in bounded request or stream objects and never enters the
governed metadata context. A continuation keeps its valid affinity and pinned
revisions unless an explicit policy prohibition or provider revocation applies.
Fallback remains inside the original eligible set and stops after response
commit.

In the current single-adapter process, the eligible set contains only targets
that the attached adapter can execute. A future heterogeneous broker may own
multiple adapter processes, but it must preserve the same immutable decision,
audit and precommit-only fallback contract.

## Canonical Control-Plane Pipeline

~~~text
canonical admin request
-> authenticate and authorize control-plane scope
-> tenant and resource binding
-> idempotency key and optimistic concurrency
-> parse and bounded validation
-> compile, static analysis, fingerprint
-> immutable draft or revision persistence
-> maker-checker approval and quorum
-> atomic activation plus audit and outbox
-> invalidation publication
-> gateway verification and last-known-good swap
-> stable redacted response
~~~

Malformed or unapproved revisions never evict last-known-good state.

## Ownership and Dependency Direction

| Responsibility | Owner |
| --- | --- |
| Channel parsing and protocol adaptation | prodex-cli and existing channel crates |
| Canonical HTTP boundary | prodex-gateway-http, prodex-gateway-core, prodex-gateway-server |
| Authentication and authorization | prodex-authn, prodex-authz, prodex-domain |
| Pure governance types and decisions | prodex-domain |
| Inspection and policy orchestration | prodex-application |
| Presidio network adapter | prodex-presidio |
| Provider contracts and pure routing | prodex-provider-spi and prodex-provider-core |
| Streaming, affinity, and transport | existing runtime proxy crates |
| Durable contracts and adapters | prodex-storage and SQLite/PostgreSQL adapters |
| Ephemeral coordination | Redis adapters |
| Secrets | prodex-domain SecretRef and prodex-secret-store |
| Audit and telemetry adapters | prodex-audit-log and prodex-observability |
| Composition and compatibility | prodex-app |

Domain, PDP, and routing planner code cannot depend on HTTP frameworks,
databases, filesystems, environment variables, network clients, or async
runtimes. Provider and inspection adapters cannot own business policy.

## Operating Modes

| Mode | Required behavior |
| --- | --- |
| personal | Backward-compatible local behavior; governance may be off or advisory |
| enterprise_observe | Governance computes immutable shadow decisions; compatible behavior remains authoritative |
| enterprise_enforce | Identity, inspection, policy, obligations, and governed routing are enforced |
| bank_enforce | Fail closed for missing identity/audit/policy/registry, unresolved classification, raw secrets, and unapproved providers |

Capabilities move through off, observe, and enforce using tenant-scoped,
revisioned state. A feature flag cannot disable a mandatory bank control without
an authorized, expiring, audited break-glass decision.

## Five-Phase Migration

~~~mermaid
flowchart LR
    BASE[Clean baseline and inventory]
    P1[Phase 1\nOne inspection boundary]
    P2[Phase 2\nClassification and response guard]
    P3[Phase 3\nPDP, approvals, policy store, audit]
    P4[Phase 4\nGoverned provider routing]
    P5[Phase 5\nUnified gateway and bank hardening]
    DONE[Governed production pipeline]

    BASE --> P1 --> P2 --> P3 --> P4 --> P5 --> DONE
    P1 -. rollback .-> BASE
    P2 -. rollback .-> P1
    P3 -. LKG rollback .-> P2
    P4 -. registry and score rollback .-> P3
    P5 -. deployment rollback .-> P4
~~~

Each phase exits only after targeted tests, workspace gates, evidence updates,
and a reversible cutover. Compatibility adapters have an owner, telemetry, a
test, and a removal condition.
