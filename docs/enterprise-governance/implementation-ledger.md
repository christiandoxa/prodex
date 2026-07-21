# Enterprise Governance Implementation Ledger

Last updated: 2026-07-14 (Asia/Jakarta).

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
| B-01 | Clean current baseline before production changes | tested | Current tranche baseline HEAD `8ca79a62`; candidate evidence is the working diff from that revision |
| B-02 | Read architecture, threat, policy, deployment, provider, storage, and prior refactor evidence | tested | `00-baseline-and-inventory.md`, `01-target-architecture.md`, ADRs 0001-0010, and the machine-readable security matrix |
| B-03 | Record host and toolchain | tested | Linux 6.17; Rust/Cargo 1.97.0; Node 24.18.0; npm 12.0.1; 16 logical CPUs; 29 GiB RAM |
| B-04 | Run formatting baseline | tested | cargo fmt --check passed |
| B-05 | Run Clippy baseline | tested | locked workspace/all-targets/all-features with warnings denied passed |
| B-06 | Run Rust test baseline | tested | Candidate full workspace passed 2,835 tests across 189 suites |
| B-07 | Run npm install/test baseline | tested | `npm test` passed; `npm ci` is inapplicable because no lockfile is tracked and generated cross-platform workspace locks reject single-host installation |
| B-08 | Run documentation and architecture guards | tested | docs lint, crate boundary, and deployment security guard passed |
| B-09 | Capture baseline performance and resource evidence | tested | Current `8ca79a62` governance hot-path benchmark passed with 100-sample maximum-bound estimate intervals of 27.143-27.238 us inspection, 2.768-2.796 us PDP and 6.290-6.393 us routing; prior CPU-pinned `e308fdf6` comparison passed all eight disabled p95/p99 budgets |

## Phase 1 — Inspection Boundary

| ID | Requirement | Status | Evidence or next proof |
| --- | --- | --- | --- |
| P1-01 | Inventory every ingress, schema, stream, provider, session, policy, audit, and storage path | tested | `00-baseline-and-inventory.md` and `scripts/ci/production-boundary-guard.mjs` |
| P1-02 | Map all Presidio calls and duplicate redaction policy | tested | `gateway_presidio_redaction_failure_is_audited_without_payload_or_endpoint_leakage` and the production-boundary guard |
| P1-03 | Pure bounded domain inspection contract | tested | `inspection_result_is_bounded_deterministic_and_content_free` and `application_inspection_rejects_unbounded_detector_sources` |
| P1-04 | One application inspection use case | tested | `application_inspection_combines_sources_monotonically` and `application_pipeline_classifies_before_policy_evaluation` |
| P1-05 | Presidio remains an adapter, not policy owner | tested | `production-boundary-guard.mjs`; orchestration is in `prodex-application`, network adaptation in `prodex-presidio`/runtime adapter code |
| P1-06 | Trusted enterprise/bank Presidio endpoint policy | tested | `enterprise_endpoints_require_private_or_explicitly_trusted_hosts` and `presidio_client_does_not_follow_redirects_with_inspected_content` |
| P1-07 | Schema-aware request inspection and deterministic masking | tested | `walker_inspects_supported_nested_fields_without_retaining_argument_keys` and `local_inspection_masks_supported_nested_content_and_preserves_structure` |
| P1-08 | Detect PII, credentials, tokens, private keys, and financial identifiers | tested | `local_inspection_masks_supported_nested_content_and_preserves_structure` and `malformed_private_key_is_masked_through_end_of_value` |
| P1-09 | Bound input, depth, detectors, patterns, findings, response, timeout, and concurrency | tested | `walker_bounds_value_count_and_total_text`, `runtime_presidio_config_bounds_external_work`, and `local_inspection_rejects_deep_and_match_flood_inputs` |
| P1-10 | Typed mode-specific failure behavior | tested | `governance_modes_preserve_personal_compatibility_and_bank_fail_closed_rules` and `bank_snapshot_denies_unsupported_inspection` |
| P1-11 | Low-cardinality inspection telemetry | tested | `inspection_metrics_are_bounded_and_low_cardinality` |
| P1-12 | Phase 1 schema, Unicode, flood, endpoint, failure, leakage, concurrency, and compatibility tests | tested | Includes `runtime_presidio_detector_failure_matrix_is_bounded_and_content_preserving` and `tenant_patterns_are_isolated_and_support_unicode_interior_globs` |
| P1-13 | Remove duplicate production PII policy paths | implemented | Production application admission consumes one typed inspection plan; compatibility redactors remain adapter implementations, not independent policy authorities |
| P1-X | Phase 1 exit: one production inspection boundary | tested | HTTP, compact, SSE, WebSocket, Gemini and supported provider routes pass the application boundary and guards |

## Phase 2 — Classification and Bidirectional Guardrails

| ID | Requirement | Status | Evidence or next proof |
| --- | --- | --- | --- |
| P2-01 | Four-level monotonic classification model | tested | `classification_is_deterministic_and_monotonic` |
| P2-02 | Versioned compiled classification rules with checksum, activation, rollback, and LKG | tested | Canonical checksum plus generic SQLite/PostgreSQL lifecycle, activation, LKG and rollback tests |
| P2-03 | Trusted labels can raise; audited authorization required to lower | tested | `classification_and_coverage_only_move_conservatively` and `rule_publication_rejects_duplicate_or_weakened_rules` |
| P2-04 | Unsupported or partial coverage is explicit policy input | tested | `inspection_failure_mode_matrix_preserves_shadow_and_fail_closed_semantics` covers unsupported modalities and detector failures across observe, enforce, and bank modes |
| P2-05 | Typed request and response obligations | tested | `obligation_matrix_preserves_classification_and_observe_enforce_semantics` and `request_and_session_obligations_return_stable_typed_violations` |
| P2-06 | Structured request masking preserves provider schemas | tested | `mask_obligation_requires_explicit_masking_evidence` and schema-aware walker tests |
| P2-07 | Incremental response inspection across stream chunks | tested | Exhaustive boundaries plus `incremental_inspector_randomized_sse_and_websocket_chunk_corpus` cover arbitrary SSE bytes, valid UTF-8 WebSocket frames, and Unicode splits |
| P2-08 | Correct pre-commit denial and post-commit termination/accounting | tested | `incremental_inspector_commit_outcome_is_stable_across_transport_chunking`, `provider_retry_boundary_marks_irreversible_stages_committed`, and precommit-only retry regressions |
| P2-09 | Immutable governed request metadata context | tested | `application-boundary-guard.mjs` and redacted `Debug` tests in governance/application types |
| P2-10 | Session classification monotonicity, binding, timeout, and revocation hooks | tested | Bounded memory snapshots, synchronous durable security-relevant updates, coalesced timestamp touches, SQLite/PG storage tests, `session_context_propagates_age_idle_classification_and_affinity`, `session_reuse_with_another_principal_is_revoked`, and revoke-route HTTP regression |
| P2-11 | Phase 2 matrix, property, fuzz, and stream tests | tested | Deterministic SSE/WebSocket-equivalent chunk corpus plus 31-second, 117,401-execution governance-policy fuzz run |
| P2-X | Phase 2 exit: every routed request has classification and coverage | tested | Enforcing application admission requires typed classification and coverage before routing |

## Phase 3 — PDP, Policy Store, Approval, Audit, and SIEM

| ID | Requirement | Status | Evidence or next proof |
| --- | --- | --- | --- |
| P3-01 | Typed PolicyInput, PolicyDecision, effects, obligations, and stable reasons | tested | `explicit_deny_wins_and_drops_obligations` and `missing_or_cross_tenant_attributes_fail_closed` |
| P3-02 | Deterministic, side-effect-free, bounded, network-free evaluator | tested | `policy_compilation_is_bounded_and_rejects_duplicate_ids` and the production boundary guard |
| P3-03 | PAP, PIP, PDP, and all PEP responsibilities are explicit | implemented | `01-target-architecture.md`, ADR 0002, application governance lifecycle and obligation executors |
| P3-04 | RBAC and ABAC attributes with explicit-deny precedence | tested | `principal_evidence_enforces_required_scope_and_anonymous_policy`, `explicit_deny_wins_and_drops_obligations`, typed group/department selectors, runtime mapping and same-tenant SCIM cache tests |
| P3-05 | Versioned policy drafts, immutable revisions, active/LKG pointers, history, rollback | tested | All four artifact kinds share SQLite/PostgreSQL repository contracts and HTTP routes; live PostgreSQL all-kind lifecycle passed |
| P3-06 | Parse, validate, compile, analyze, fingerprint, approve, activate, invalidate, verify | tested | Gateway maker-checker/CAS/LKG regression plus SQLite and live PostgreSQL lifecycle proofs |
| P3-07 | Maker-checker approval state machine and quorum | tested | `maker_checker_quorum_and_activation_are_enforced`, `approval_replay_is_idempotent_and_stale_versions_fail`, and gateway lifecycle test |
| P3-08 | Narrow break-glass with expiry and revocation | tested | `gateway_break_glass_http_enforces_scope_expiry_revocation_and_audit` covers independent quorum, exact scope, expiry, revocation, bounded purge authorization, and durable audit integrity |
| P3-09 | Optional execution approval without raw prompt retention | tested | `execution_fingerprint_is_stable_and_bound_to_the_request_context`, `execution_approval_is_policy_selected_quorum_gated_and_one_use`, and gateway approval HTTP tests cover digest binding, quorum, single use, and content-free administration; break-glass remains a distinct narrower credential |
| P3-10 | One tamper-evident audit contract for data and control planes | implemented | Typed tenant chain/storage contract, bounded background commit-ack writer, explainable content-free decision context, integrity endpoint and bounded audited export exist; database/queue availability remains a pre-dispatch dependency |
| P3-11 | Durable SIEM outbox, retry, deduplication, dead letter, and lag | tested | SQLite exporter retry/dead-letter tests and live PostgreSQL leased claim/finalize lifecycle passed |
| P3-12 | Mandatory audit failure matrix and bank fail-closed behavior | tested | Failure rollback regression, live PostgreSQL transaction proof and bank fail-closed matrix |
| P3-13 | Complete CLI/API control-plane policy and audit interfaces | implemented | Four-kind admin lifecycle, integrity, outbox health, bounded audit export, legal-hold list/upsert/release, retention purge, break-glass approval, and hash/current-session revoke routes are tenant-scoped |
| P3-14 | Phase 3 golden, adversarial, race, storage, approval, audit, and outbox tests | tested | Domain, HTTP, SQLite and live PostgreSQL lifecycle/outbox suites passed |
| P3-X | Phase 3 exit: PDP/store sole policy source and material decisions audited | tested | Enforcing runtime uses governed snapshots and pre-dispatch durable audit acknowledgement |

## Phase 4 — Governed Provider Routing

| ID | Requirement | Status | Evidence or next proof |
| --- | --- | --- | --- |
| P4-01 | Revisioned tenant-aware provider registry | tested | `governed_routing_never_places_ineligible_or_cross_tenant_routes_in_fallback` |
| P4-02 | SecretRef-only provider credentials | implemented | Existing provider invocation supports SecretRef; registry contract still required |
| P4-03 | Hard eligibility filtering before scoring | tested | `governed_routing_enforces_every_hard_eligibility_gate` |
| P4-04 | Deterministic fixed-point soft scoring and tie-break | tested | `governed_routing_scores_in_fixed_point_and_breaks_ties_by_provider` |
| P4-05 | Bounded redacted score breakdown and reason codes | tested | `governed_routing_explains_every_hard_filter_with_stable_reason_codes`, bounded/debug tests |
| P4-06 | Endpoint-aware circuit breaker and background health | implemented | Existing transport primitives cover parts; governance snapshot integration remains |
| P4-07 | Eligible-set-only pre-commit retry/fallback and DR | tested | `governed_routing_runtime_signals_change_selection_and_keep_only_eligible_fallbacks`, `provider_registry_resolves_selected_heterogeneous_projected_adapter`, and precommit-only fallback regressions cover dynamic heterogeneous dispatch without postcommit retry; managed regional DR remains deployment evidence |
| P4-08 | Continuation policy pinning and provider revocation | tested | `governed_routing_does_not_preserve_affinity_after_revocation` and `explicit_provider_revocation_overrides_continuation_affinity` |
| P4-09 | Versioned pricing, reservation, estimate, and reconciliation | tested | Provider-registry schema v2 publishes per-model/default input/output prices, admission pins the pricing revision and authoritative estimate, reconciliation uses the same pinned cost, and expired durable reservations recover idempotently in SQLite/PostgreSQL workers |
| P4-10 | Shared provider SPI and capability-gated unsupported adapters | tested | Provider registry advertises only executable adapter capabilities; `provider_registry_resolves_selected_heterogeneous_projected_adapter` and `configured_provider_reference_reaches_application_invocation` |
| P4-11 | Shadow, canary, hard-filter, score, then legacy removal migration | implemented | Observe/enforce modes exist; rollout/release evidence remains pending |
| P4-12 | Phase 4 matrix/property/affinity/revocation/outage/cost/load tests | tested | `crates/prodex-provider-spi/tests/governed_routing.rs` named matrix and boundary tests |
| P4-X | Phase 4 exit: every dispatch has one auditable routing decision | tested | Production guard, dispatch revalidation and mandatory audit regressions passed |

## Phase 5 — Unified Gateway and Bank Hardening

| ID | Requirement | Status | Evidence or next proof |
| --- | --- | --- | --- |
| P5-01 | CLI, IDE, API, and supported machine channels share one authenticated application boundary | tested | Production/application boundary guards cover forwarded HTTP, compact, SSE, WebSocket and Gemini routes |
| P5-02 | OIDC PKCE/device or supported human flow, bearer validation, service identity, and mTLS | tested | Gateway browser Authorization Code with PKCE S256, Redis-shared one-time state and sessions, secure cookie login/callback/logout, signed back-channel logout, workload JWT/JWKS/scope verification, Rustls client-certificate validation, and JWT `cnf.x5t#S256` peer binding are wired and covered by focused runtime tests; deployment IdP/PKI rotation remains acceptance evidence |
| P5-03 | Canonical routes, limits, deadlines, concurrency, distributed rate/quota, overload | implemented | Existing gateway controls cover much of this; governance sequence integration remains |
| P5-04 | Trusted proxies, safe client metadata, browser CSRF/Origin/Host/cookies | tested | Edge tests prove peer preservation, exact trusted proxies, bounded client derivation, forwarding-header stripping, explicit non-loopback Host authority, PKCE state/nonce validation, Secure/HttpOnly/SameSite browser sessions, Redis-backed multi-replica continuity, and local plus signed back-channel logout invalidation |
| P5-05 | Typed session binding, timeouts, revocation, concurrency, re-auth/MFA, network risk, revision pinning | tested | Authority hydration, atomic admission/revoke+audit/outbox, owner-bound current-session self-revoke, and `cross_replica_revocation_epoch_invalidates_cached_sessions_promptly` prove shared-authority invalidation; deployed two-gateway chaos and re-auth/MFA remain pending |
| P5-06 | PostgreSQL authority, RLS, transaction tenant context, external migrations | tested | Disposable live PostgreSQL/TLS proof passed four runtime tests, migration/RLS guards and cross-tenant checks |
| P5-07 | Redis only for rebuildable ephemeral coordination | tested | Existing architecture and guards enforce non-authoritative use |
| P5-08 | SQLite local compatibility and enterprise migration tests | tested | `all_governance_artifact_kinds_use_revisioned_authority`, cross-tenant/CAS/LKG/audit/outbox tests |
| P5-09 | External secret/Vault-compatible provider, leases, rotation, TLS identity, zeroization | tested | Bounded projected-secret adapter is the production Vault Agent/CSI boundary; direct Vault HTTP authority is intentionally unsupported |
| P5-10 | Append-only durable audit and SIEM exporter operations | tested | Bounded audit writer/export API, SQLite exporter tests and live PostgreSQL outbox operations passed |
| P5-11 | Low-cardinality metrics, alerts, SLOs, and runbooks | implemented | Live authn, authz, tenant-isolation, policy, and secret-provider counters feed the checked-in Prometheus rules and Grafana dashboard; pager routing and achieved environment SLOs remain deployment evidence |
| P5-12 | Hardened Compose/Kubernetes, least privilege, HA, drain, deny-default network policy | implemented | Existing artifacts cover much; bank governance/Vault/SIEM egress and tests remain |
| P5-13 | Encrypted backup, isolated restore, audit/policy/registry verification, and DR drills | tested | AES-256-GCM disposable PostgreSQL backup/isolated restore passed with final synthetic RPO 1.813 s and RTO 1.280 s; tenant/governance fingerprints, RLS, policy/provider/session/audit/SIEM links remained intact; production KMS/PITR and regional cutover remain deployment evidence |
| P5-14 | Phase 5 identity/session/RLS/Vault/audit/deployment/restore/chaos tests | implemented | Identity/session/config/deployment, live PostgreSQL/RLS/SIEM, fuzz, stress and restore gates passed; two-gateway chaos remains deployment acceptance work |
| P5-X | Phase 5 exit: governed channel parity and tested bank profile | implemented | Governed HTTP, browser OIDC, workload JWT, and direct mTLS channels pass in-process guards; managed IdP/PKI rotation, failover, external SIEM delivery, and deployed multi-replica chaos remain environment acceptance blockers |

## Cross-Cutting Security Controls

| ID | Requirement | Status | Evidence or next proof |
| --- | --- | --- | --- |
| S-01 | No raw content, sensitive values, secrets, tokens, credentials, or full IPs in operational surfaces | tested | Redacted governance `Debug`, webhook stable-reason regression, runtime log redaction and observability guard |
| S-02 | All network-facing resources and cardinality are bounded | tested | Inspection/provider/tool/evidence bounds plus production and observability boundary guards |
| S-03 | PDP and routing planner perform no I/O | tested | Production/application boundary guards |
| S-04 | Channel, adapter, alias, or compatibility routes cannot bypass policy/classification | tested | Production/application/provider-SPI boundary guards |
| S-05 | Bank mode rejects raw secret configuration | tested | Bank deployment matrix and projected-secret configuration guards |
| S-06 | Fallback never leaves original hard eligibility set | tested | `governed_routing_never_places_ineligible_or_cross_tenant_routes_in_fallback` |
| S-07 | No retry or rotation after commit | tested | Existing runtime regressions; retain through governed router |
| S-08 | No request-path panic on external input/config | tested | Production boundary guard and bounded invalid-input tests |
| S-09 | Unsafe code remains narrowly bounded | tested | Crate policy and existing CI guards |
| S-10 | No arbitrary policy script or unbounded/ReDoS regex | tested | Typed bounded rule compiler and bounded glob/pattern publication tests |
| S-11 | Forwarded network metadata trusted only from configured proxies | implemented | Existing gateway controls; bind into policy input |
| S-12 | Raw prompt/response retention disabled by default | tested | `gateway_release_secret_canaries_only_reach_the_authorized_upstream` scans every local runtime artifact after an authorized request and proves provider, virtual-key, admin, prompt, and response canaries are absent while authorized prompt/response content still reaches its intended boundary |
| S-13 | Mandatory controls have explicit failure matrices; bank security controls do not fail open | implemented | ADRs 0007/0010, operations failure runbooks and bank config matrix |
| S-14 | Stable redacted local errors reveal no tenant/provider/policy secrets | tested | Governed routing debug test, webhook stable-reason test and error-envelope tests |
| S-15 | Threat tests cover confusion, bypass, stale state, replay, insider, injection, exfiltration, compromise, SSRF, smuggling, Unicode, tampering, leakage, DNS/egress, and DoS | tested | Machine-readable matrix, security guards, fuzz, stress and live storage proofs; deployment-specific two-replica chaos is explicitly not claimed |

## Performance, Storage, Configuration, and Quality Gates

| ID | Requirement | Status | Evidence or next proof |
| --- | --- | --- | --- |
| Q-01 | Immutable read-optimized policy/registry/config snapshots | tested | Independent tenant ArcSwap authorities retain active/LKG snapshots and fail safely on invalid refresh |
| Q-02 | No request-path compilation, probe, storage revision read, Vault, SIEM, DNS, or external PDP call | implemented | Pure PDP/routing and memory-only session lookup; new/security-relevant session writes and mandatory audit use bounded background workers with pre-dispatch acknowledgements; policy-selected Presidio/webhook calls remain request-path exceptions |
| Q-03 | Preserve streaming with bounded windows and one deadline | tested | Incremental response guard and precommit-only retry regressions |
| Q-04 | Required governance Criterion/integration/load benchmarks | implemented | Maximum inspection/PDP/routing, disabled compatibility, local load and stress passed; external multi-replica soak and audit/session pressure remain deployment evidence |
| Q-05 | Acceptance budgets: disabled <=5 percent regression; governance <=5 ms p99; PDP <=1 ms; routing <=2 ms | tested | Worst disabled p99 delta +4.4%; PDP 2.066 us p99; routing 6.530 us p99 |
| Q-06 | Versioned SQLite/PostgreSQL migrations for all governance entities | tested | SQLite runtime repository and live PostgreSQL migration/lifecycle proofs passed |
| Q-07 | Transactional activation, audit, and outbox where required | tested | SQLite and live PostgreSQL repository proofs passed |
| Q-08 | Typed versioned governance modes and capability rollout states | tested | `governance_modes_preserve_personal_compatibility_and_bank_fail_closed_rules` and runtime-policy mode tests |
| Q-09 | Bank startup rejects insecure configuration combinations | tested | `bank_governance_deployment_matrix_fails_closed` and deployment security guard |
| Q-10 | Unit/golden/property/fuzz/integration/security/chaos program | implemented | Unit/golden/property/fuzz/integration/security/stress gates passed; deployment-specific multi-replica chaos remains |
| Q-11 | CI architecture and deployment guards from the specification | tested | Production, application, auth, config, provider-SPI, deployment-security and observability guards |
| Q-12 | Full format, lint, test, npm, docs, audit, deny, fuzz, load, migration, restore, storage-replica, and benchmark gates | tested | Final report records passing commands; managed failover, external SIEM and production multi-replica soak remain deployment acceptance evidence |

## Required Artifacts

| Artifact | Status |
| --- | --- |
| 00-baseline-and-inventory.md | tested |
| 01-target-architecture.md | tested |
| 02-trust-boundaries-and-data-flow.md | tested |
| 03-data-classification-and-inspection.md | implemented |
| 04-policy-model-and-obligations.md | implemented |
| 05-approval-and-break-glass.md | implemented |
| 06-provider-registry-and-routing.md | implemented |
| 07-identity-session-and-api-gateway.md | implemented |
| 08-audit-siem-and-secret-management.md | implemented |
| 09-storage-ha-backup-and-dr.md | implemented |
| 10-rollout-rollback-and-compatibility.md | implemented |
| 11-security-test-matrix.md | implemented |
| 12-performance-baseline-and-results.md | implemented |
| 13-operator-runbooks.md | implemented |
| Threat model and enterprise readiness updates | tested |
| Classification/inspection ADR | implemented |
| PDP/PAP/PIP/PEP snapshot ADR | implemented |
| Policy approval/activation/LKG ADR | implemented |
| Execution approval ADR if enabled | implemented |
| Provider registry/routing ADR | implemented |
| Continuation pinning/revocation ADR | implemented |
| Mandatory audit/SIEM outbox ADR | implemented |
| Session/trusted proxy ADR | implemented |
| External secret/Vault ADR | implemented |
| Bank profile/fail-closed ADR | implemented |
| Synthetic sample configurations and policies | implemented |
| Machine-readable security test matrix | implemented |
| Final implementation and verification report | implemented |

## Completion Rule

The program is complete only after each phase exit row and every cross-cutting
row is tested, all required artifacts exist, all supported channels use the
governed production pipeline, superseded authoritative paths are removed, and
the final command/benchmark evidence is recorded without overstating
environmental or legal assurance.
