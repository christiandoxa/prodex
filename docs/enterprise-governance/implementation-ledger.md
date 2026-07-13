# Enterprise Governance Implementation Ledger

Last updated: 2026-07-13 (Asia/Jakarta).

This ledger is the authoritative progress index for the five-phase enterprise
governance program. Status values are:

- planned: required work has not started;
- in progress: implementation or verification is active;
- blocked: an external dependency prevents progress and has an owner;
- implemented: production behavior exists but its full gate is not yet proven;
- tested: implementation and required evidence both pass;
- deferred: an owner, reason, and activation condition are recorded.

No row is complete merely because adjacent infrastructure exists.

## Baseline Evidence

| ID | Requirement | Status | Evidence or next proof |
| --- | --- | --- | --- |
| B-01 | Clean current baseline before production changes | tested | HEAD 9bdde846 on main; worktree matched origin/main with no diff |
| B-02 | Read architecture, threat, policy, deployment, provider, storage, and prior refactor evidence | in progress | Core documents inspected; focused inventories run in parallel |
| B-03 | Record host and toolchain | tested | Linux 6.17; Rust/Cargo 1.97.0; Node 24.18.0; npm 12.0.1; 16 logical CPUs; 29 GiB RAM |
| B-04 | Run formatting baseline | tested | cargo fmt --check passed |
| B-05 | Run Clippy baseline | tested | locked workspace/all-targets/all-features with warnings denied passed |
| B-06 | Run Rust test baseline | implemented | Full workspace reached two known concurrent cache-stat assertion failures; the affected crate passed serially, 3 tests |
| B-07 | Run npm install/test baseline | in progress | npm ci is blocked by the intentionally absent package-lock.json; npm test running |
| B-08 | Run documentation and architecture guards | tested | docs lint, crate boundary, and deployment security guard passed |
| B-09 | Capture baseline performance and resource evidence | planned | Run existing benchmark/load commands before governance hot-path changes |

## Phase 1 — Inspection Boundary

| ID | Requirement | Status | Evidence or next proof |
| --- | --- | --- | --- |
| P1-01 | Inventory every ingress, schema, stream, provider, session, policy, audit, and storage path | in progress | 00-baseline-and-inventory.md plus parallel audits |
| P1-02 | Map all Presidio calls and duplicate redaction policy | tested | Local pipeline runs Presidio and a second runtime guardrail redactor; Gemini Live can dispatch before body capture |
| P1-03 | Pure bounded domain inspection contract | planned | Add classification, coverage, finding, detector, location, tag, reason, and revision types without I/O dependencies |
| P1-04 | One application inspection use case | planned | Combine adapter observations and local findings at the application boundary |
| P1-05 | Presidio remains an adapter, not policy owner | implemented | Existing prodex-presidio adapter exists; production orchestration still lives in prodex-app and needs consolidation |
| P1-06 | Trusted enterprise/bank Presidio endpoint policy | planned | Current validation allows HTTP(S) host URLs but does not prove trusted/on-prem egress |
| P1-07 | Schema-aware request inspection and deterministic masking | planned | Replace concatenate-and-rewrite flow with bounded supported-field walking |
| P1-08 | Detect PII, credentials, tokens, private keys, and financial identifiers | planned | Define bounded local detectors and map Presidio kinds without storing matches |
| P1-09 | Bound input, depth, detectors, patterns, findings, response, timeout, and concurrency | planned | Existing 10-second and 64-MiB response bounds are insufficient |
| P1-10 | Typed mode-specific failure behavior | planned | Personal compatibility; bank required inspection fails closed |
| P1-11 | Low-cardinality inspection telemetry | planned | Duration, coverage, finding category, masking, timeout, error, denial |
| P1-12 | Phase 1 schema, Unicode, flood, endpoint, failure, leakage, concurrency, and compatibility tests | planned | Add focused unit/integration/load evidence |
| P1-13 | Remove duplicate production PII policy paths | planned | Remove only after all supported data-plane routes converge |
| P1-X | Phase 1 exit: one production inspection boundary | planned | Must include HTTP, compact, supported provider routes, and WebSocket paths |

## Phase 2 — Classification and Bidirectional Guardrails

| ID | Requirement | Status | Evidence or next proof |
| --- | --- | --- | --- |
| P2-01 | Four-level monotonic classification model | planned | Public, Internal, Confidential, Restricted |
| P2-02 | Versioned compiled classification rules with checksum, activation, rollback, and LKG | planned | Reuse policy snapshot/config publication primitives |
| P2-03 | Trusted labels can raise; audited authorization required to lower | planned | Add explicit declassification operation only if enabled |
| P2-04 | Unsupported or partial coverage is explicit policy input | planned | Unknown cannot route in enforcement modes |
| P2-05 | Typed request and response obligations | planned | Mask/deny, tools, models, modalities, tokens, context, audit, retention, locality, residency |
| P2-06 | Structured request masking preserves provider schemas | planned | Cover tool arguments and supported uploaded text |
| P2-07 | Incremental response inspection across stream chunks | implemented | Existing keyword reader has a bounded tail; replace with typed coverage and obligations |
| P2-08 | Correct pre-commit denial and post-commit termination/accounting | planned | Preserve upstream transparency and no synthetic upstream error |
| P2-09 | Immutable governed request metadata context | planned | No service locator and no raw content |
| P2-10 | Session classification monotonicity, binding, timeout, and revocation hooks | planned | Reuse session and identity primitives |
| P2-11 | Phase 2 matrix, property, fuzz, and stream tests | planned | Cover four classifications, three coverage states, transports, tools, provider locality, and modes |
| P2-X | Phase 2 exit: every routed request has classification and coverage | planned | Production evidence required |

## Phase 3 — PDP, Policy Store, Approval, Audit, and SIEM

| ID | Requirement | Status | Evidence or next proof |
| --- | --- | --- | --- |
| P3-01 | Typed PolicyInput, PolicyDecision, effects, obligations, and stable reasons | planned | Existing policy snapshot is integrity/LKG infrastructure, not a PDP |
| P3-02 | Deterministic, side-effect-free, bounded, network-free evaluator | planned | Reuse typed domain boundary; no scripting or external request-path PDP |
| P3-03 | PAP, PIP, PDP, and all PEP responsibilities are explicit | planned | Data and control flow documented in target architecture |
| P3-04 | RBAC and ABAC attributes with explicit-deny precedence | planned | Reuse authn/authz domain types; add classification, session, environment, capability attributes |
| P3-05 | Versioned policy drafts, immutable revisions, active/LKG pointers, history, rollback | planned | Add SQLite/PostgreSQL migrations and storage contracts |
| P3-06 | Parse, validate, compile, analyze, fingerprint, approve, activate, invalidate, verify | planned | Publication rejects unknown, ambiguous, cyclic, contradictory, overflowing, or unbounded input |
| P3-07 | Maker-checker approval state machine and quorum | planned | Include expiry, cancellation, rejection, self-approval denial, tenant/scope binding, idempotency, optimistic concurrency |
| P3-08 | Narrow break-glass with expiry and review | implemented | Generic break-glass primitives exist; governance activation binding remains |
| P3-09 | Optional execution approval without raw prompt retention | planned | Enable only with exact policy and encrypted TTL design |
| P3-10 | One tamper-evident audit contract for data and control planes | planned | Typed control-plane chain exists; live data-plane still uses best-effort legacy file events |
| P3-11 | Durable SIEM outbox, retry, deduplication, dead letter, and lag | planned | Remote SIEM never blocks data path |
| P3-12 | Mandatory audit failure matrix and bank fail-closed behavior | planned | Document and test operation-specific outcomes |
| P3-13 | Complete CLI/API control-plane policy and audit interfaces | planned | Include auth scope, OpenAPI, idempotency, and preconditions |
| P3-14 | Phase 3 golden, adversarial, race, storage, approval, audit, and outbox tests | planned | Current primitives do not prove the requested lifecycle |
| P3-X | Phase 3 exit: PDP/store sole policy source and material decisions audited | planned | Production cutover evidence required |

## Phase 4 — Governed Provider Routing

| ID | Requirement | Status | Evidence or next proof |
| --- | --- | --- | --- |
| P4-01 | Revisioned tenant-aware provider registry | planned | Existing provider catalog/SPI lacks approved governance descriptors |
| P4-02 | SecretRef-only provider credentials | implemented | Existing provider invocation supports SecretRef; registry contract still required |
| P4-03 | Hard eligibility filtering before scoring | planned | Tenant, classification, residency, retention, capability, credential, status, circuit, quota, and policy obligations |
| P4-04 | Deterministic fixed-point soft scoring and tie-break | planned | Risk, health, latency, load, cost, quota headroom, priority, affinity |
| P4-05 | Bounded redacted score breakdown and reason codes | planned | Include policy, registry, pricing, and score revisions |
| P4-06 | Endpoint-aware circuit breaker and background health | implemented | Existing transport primitives cover parts; governance snapshot integration remains |
| P4-07 | Eligible-set-only pre-commit retry/fallback and DR | planned | Preserve current no-mid-stream and affinity invariants |
| P4-08 | Continuation policy pinning and provider revocation | planned | Define new-request, continuation, and active-stream behavior |
| P4-09 | Versioned pricing, reservation, estimate, and reconciliation | implemented | Existing atomic accounting exists; governed provider/model pricing revisions remain |
| P4-10 | Shared provider SPI and capability-gated unsupported adapters | implemented | Existing OpenAI, Anthropic, Gemini, local-compatible contracts need registry conformance proof |
| P4-11 | Shadow, canary, hard-filter, score, then legacy removal migration | planned | Never leave two authoritative routers |
| P4-12 | Phase 4 matrix/property/affinity/revocation/outage/cost/load tests | planned | Add governed-routing-specific evidence |
| P4-X | Phase 4 exit: every dispatch has one auditable routing decision | planned | Production evidence required |

## Phase 5 — Unified Gateway and Bank Hardening

| ID | Requirement | Status | Evidence or next proof |
| --- | --- | --- | --- |
| P5-01 | CLI, IDE, API, and supported machine channels share one authenticated application boundary | planned | Current local compatibility and early WebSocket paths require convergence |
| P5-02 | OIDC PKCE/device or supported human flow, bearer validation, service identity, and mTLS | implemented | OIDC/service primitives exist; complete channel parity and bank mTLS evidence remain |
| P5-03 | Canonical routes, limits, deadlines, concurrency, distributed rate/quota, overload | implemented | Existing gateway controls cover much of this; governance sequence integration remains |
| P5-04 | Trusted proxies, safe client metadata, browser CSRF/Origin/Host/cookies | implemented | Existing admin/expose controls exist; unified gateway proof remains |
| P5-05 | Typed session binding, timeouts, revocation, concurrency, re-auth/MFA, network risk, revision pinning | planned | Session metadata exists but not the full governance contract |
| P5-06 | PostgreSQL authority, RLS, transaction tenant context, external migrations | implemented | Existing storage baseline is substantial; new governance entities and proofs remain |
| P5-07 | Redis only for rebuildable ephemeral coordination | tested | Existing architecture and guards enforce non-authoritative use |
| P5-08 | SQLite local compatibility and enterprise migration tests | implemented | Extend for governance tables |
| P5-09 | External secret/Vault-compatible provider, leases, rotation, TLS identity, zeroization | planned | Projected secret provider exists; Vault-compatible production adapter remains |
| P5-10 | Append-only durable audit and SIEM exporter operations | planned | Build on Phase 3 outbox |
| P5-11 | Low-cardinality metrics, alerts, SLOs, and runbooks | planned | Reuse observability contracts |
| P5-12 | Hardened Compose/Kubernetes, least privilege, HA, drain, deny-default network policy | implemented | Existing artifacts cover much; bank governance/Vault/SIEM egress and tests remain |
| P5-13 | Encrypted backup, isolated restore, audit/policy/registry verification, and DR drills | implemented | Existing Postgres drill exists; governance entities and provider/IdP/Vault/SIEM failure paths remain |
| P5-14 | Phase 5 identity/session/RLS/Vault/audit/deployment/restore/chaos tests | planned | Full bank scenario required |
| P5-X | Phase 5 exit: governed channel parity and tested bank profile | planned | Production evidence required |

## Cross-Cutting Security Controls

| ID | Requirement | Status | Evidence or next proof |
| --- | --- | --- | --- |
| S-01 | No raw content, sensitive values, secrets, tokens, credentials, or full IPs in operational surfaces | in progress | Existing redaction is strong; new governance types and tests must preserve it |
| S-02 | All network-facing resources and cardinality are bounded | in progress | Existing guards strong; governance collections and queues need explicit limits |
| S-03 | PDP and routing planner perform no I/O | planned | Architecture boundary and CI guard |
| S-04 | Channel, adapter, alias, or compatibility routes cannot bypass policy/classification | planned | Current WebSocket and anonymous compatibility gaps contradict final requirement |
| S-05 | Bank mode rejects raw secret configuration | planned | Extend typed startup validation |
| S-06 | Fallback never leaves original hard eligibility set | planned | Governed router proof |
| S-07 | No retry or rotation after commit | tested | Existing runtime regressions; retain through governed router |
| S-08 | No request-path panic on external input/config | in progress | Extend guards and adversarial tests |
| S-09 | Unsafe code remains narrowly bounded | tested | Crate policy and existing CI guards |
| S-10 | No arbitrary policy script or unbounded/ReDoS regex | planned | Publication compiler limits |
| S-11 | Forwarded network metadata trusted only from configured proxies | implemented | Existing gateway controls; bind into policy input |
| S-12 | Raw prompt/response retention disabled by default | implemented | Preserve in approval/session/audit storage |
| S-13 | Mandatory controls have explicit failure matrices; bank security controls do not fail open | planned | Phase 3/5 artifact and tests |
| S-14 | Stable redacted local errors reveal no tenant/provider/policy secrets | in progress | Extend existing error-plan pattern |
| S-15 | Threat tests cover confusion, bypass, stale state, replay, insider, injection, exfiltration, compromise, SSRF, smuggling, Unicode, tampering, leakage, DNS/egress, and DoS | planned | Machine-readable matrix required |

## Performance, Storage, Configuration, and Quality Gates

| ID | Requirement | Status | Evidence or next proof |
| --- | --- | --- | --- |
| Q-01 | Immutable read-optimized policy/registry/config snapshots | implemented | Existing ArcSwap/LKG patterns available; governance snapshots remain |
| Q-02 | No request-path compilation, probe, storage revision read, Vault, SIEM, DNS, or external PDP call | planned | External inspection allowed only when active policy explicitly requires it |
| Q-03 | Preserve streaming with bounded windows and one deadline | planned | Integrate response guard and stage budgets |
| Q-04 | Required governance Criterion/integration/load benchmarks | planned | Inspection, PDP, routing, snapshot refresh, outbox, session, circuit, and end-to-end modes |
| Q-05 | Acceptance budgets: disabled <=5 percent regression; governance <=5 ms p99; PDP <=1 ms; routing <=2 ms | planned | Five comparable samples with machine metadata |
| Q-06 | Versioned SQLite/PostgreSQL migrations for all governance entities | planned | Include tenant keys, constraints, idempotency, RLS, forward/back tests, rollback/export |
| Q-07 | Transactional activation, audit, and outbox where required | planned | Storage contract and runtime proof |
| Q-08 | Typed versioned governance modes and capability rollout states | planned | personal, enterprise_observe, enterprise_enforce, bank_enforce; off/observe/enforce |
| Q-09 | Bank startup rejects insecure configuration combinations | planned | Identity, bind, audit, classification, registry, secret, and egress checks |
| Q-10 | Unit/golden/property/fuzz/integration/security/chaos program | planned | Map every threat/control to test, mode, and phase |
| Q-11 | CI architecture and deployment guards from the specification | planned | Extend existing boundary guard suite |
| Q-12 | Full format, lint, test, npm, docs, audit, deny, fuzz, load, migration, restore, multi-replica, and benchmark gates | in progress | Record only commands actually run and environmental blockers |

## Required Artifacts

| Artifact | Status |
| --- | --- |
| 00-baseline-and-inventory.md | in progress |
| 01-target-architecture.md | in progress |
| 02-trust-boundaries-and-data-flow.md | in progress |
| 03-data-classification-and-inspection.md | planned |
| 04-policy-model-and-obligations.md | planned |
| 05-approval-and-break-glass.md | planned |
| 06-provider-registry-and-routing.md | planned |
| 07-identity-session-and-api-gateway.md | planned |
| 08-audit-siem-and-secret-management.md | planned |
| 09-storage-ha-backup-and-dr.md | planned |
| 10-rollout-rollback-and-compatibility.md | planned |
| 11-security-test-matrix.md | planned |
| 12-performance-baseline-and-results.md | planned |
| 13-operator-runbooks.md | planned |
| Threat model and enterprise readiness updates | planned |
| Classification/inspection ADR | planned |
| PDP/PAP/PIP/PEP snapshot ADR | planned |
| Policy approval/activation/LKG ADR | planned |
| Execution approval ADR if enabled | planned |
| Provider registry/routing ADR | planned |
| Continuation pinning/revocation ADR | planned |
| Mandatory audit/SIEM outbox ADR | planned |
| Session/trusted proxy ADR | planned |
| External secret/Vault ADR | planned |
| Bank profile/fail-closed ADR | planned |
| Synthetic sample configurations and policies | planned |
| Machine-readable security test matrix | planned |
| Final implementation and verification report | planned |

## Completion Rule

The program is complete only after each phase exit row and every cross-cutting
row is tested, all required artifacts exist, all supported channels use the
governed production pipeline, superseded authoritative paths are removed, and
the final command/benchmark evidence is recorded without overstating
environmental or legal assurance.
