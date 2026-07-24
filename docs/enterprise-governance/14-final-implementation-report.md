# Final Enterprise Governance Implementation Report

Date: 2026-07-14 (Asia/Jakarta)

Current tranche baseline revision: `8ca79a62`

## Report Scope

This report records implementation and focused evidence present in the release
candidate diff from the named baseline. It is not a production authorization
or regulatory certification. A row is
called tested only when a named test, guard, benchmark or artifact is recorded.

## Implementation Outcome

| Phase | Candidate outcome | Named evidence |
| --- | --- | --- |
| 1. Inspection boundary | Typed bounded inspection, schema walking, local/Presidio adapters, trusted endpoint rules and low-cardinality telemetry are implemented | `inspection_result_is_bounded_deterministic_and_content_free`; `application_inspection_combines_sources_monotonically`; Presidio/walker/local detector tests |
| 2. Classification and bidirectional obligations | Monotonic classification, explicit coverage, typed request/response/session obligations and incremental response overlap inspection are implemented | `classification_is_deterministic_and_monotonic`; `obligation_matrix_preserves_classification_and_observe_enforce_semantics`; incremental inspector tests |
| 3. PDP, lifecycle, audit and SIEM | Pure PDP, four-kind immutable governance lifecycle, maker-checker and request-digest-bound execution approval, mandatory durable decision audit, bounded audit export and durable outbox/exporter ports are implemented | policy/approval domain tests; content-free execution-approval HTTP regression; SQLite lifecycle/export tests; live PostgreSQL governance and SIEM outbox tests |
| 4. Governed provider routing | Tenant-aware hard eligibility, dynamic signals, deterministic fixed-point score, heterogeneous projected-adapter dispatch, affinity/revocation and eligible-only precommit fallback are implemented | `crates/prodex-provider-spi/tests/governed_routing.rs`; provider-registry/runtime dispatch tests; production/provider-SPI boundary guards |
| 5. Unified gateway and bank hardening | Typed identity evidence, edge security, durable governed session admission/revocation, bank startup/runtime validation and deployment guards are implemented | auth/session tests; hash/current-session revoke and audit-export HTTP lifecycle regression; `bank_governance_deployment_matrix_fails_closed`; live PostgreSQL/RLS proof and restore drill |

## Focused Verification Evidence

| Area | Evidence | Result |
| --- | --- | --- |
| Application inspection | `cargo test -q -p prodex-application --test governance_inspection` | 5 passed |
| Application obligations/pipeline | `crates/prodex-application/tests/governance_obligations.rs`, `governance_pipeline.rs` | Focused suite passed in candidate verification |
| Execution approval | `execution_fingerprint_is_stable_and_bound_to_the_request_context`; `execution_approval_is_policy_selected_quorum_gated_and_one_use`; `gateway_execution_approval_http_lists_shows_and_reviews_without_payloads` | Request digest, quorum, one use and content-free admin HTTP covered |
| Governed routing | `governed_routing_runtime_signals_change_selection_and_keep_only_eligible_fallbacks`; `provider_registry_resolves_selected_heterogeneous_projected_adapter`; precommit fallback regressions | Dynamic eligible-only heterogeneous selection and no-postcommit retry covered |
| Runtime policy/bank config | `bank_governance_deployment_matrix_fails_closed`; enforcing snapshot/mode validation tests | Focused suite passed in candidate verification |
| Governance SQLite lifecycle/session/audit/export | `cargo test -q -p prodex-storage-sqlite-runtime --test governance_repository -- --test-threads=1` | 9 passed |
| Governance PostgreSQL lifecycle/session/audit/export | `npm run ci:storage-postgres-proof`; `postgres_policy_governance_activates_and_replays_idempotently`; `postgres_governance_lifecycle_supports_all_artifact_kinds` | live Docker PostgreSQL, RLS, TLS and four runtime tests passed |
| Governance restore | `npm run ci:backup-restore-drill` | pass; AES-256-GCM backup and isolated restore measured final synthetic RPO 1.813 s and RTO 1.280 s; tenant/governance fingerprints, RLS and audit-chain links intact |
| Gateway lifecycle/session/export | `gateway_policy_http_enforces_maker_checker_replay_cas_tenant_and_lkg`; `gateway_governance_artifacts_use_generic_maker_checker_lifecycle` | Focused HTTP regressions passed |
| Cross-replica revocation epoch | `cross_replica_revocation_epoch_invalidates_cached_sessions_promptly` | Shared-authority epoch invalidates cached sessions; deployed two-gateway chaos remains pending |
| Edge Host/proxy boundary | `production_edge_security_uses_peer_trust_and_rejects_host_origin_csrf_spoofing`; `non_loopback_server_requires_explicit_expected_host` | Exact trusted proxy, forwarding stripping and explicit non-loopback Host authority covered |
| Gateway metrics | `prometheus_text_aggregates_keys_without_high_cardinality_labels` | Aggregate metrics contain no key, tenant or epoch labels |
| Tool metadata bound | `requested_tool_metadata_is_bounded` | passed |
| Webhook content safety | `guardrail_webhook_policy_reason_is_stable_and_content_free` | passed |
| Architecture/security guards | production, application, auth, config, provider-SPI, deployment-security and observability boundary guards | passed in focused verification |
| Full repository gate | `npm run test:full -- --timings` | passed for the current tranche |
| Formatting and lint | `cargo fmt --all --check`; `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings` | passed for the current tranche |
| Dependency policy | `cargo audit`; `cargo deny check advisories sources licenses bans` | no advisories or policy violations; stale skip entries emitted non-fatal warnings |
| Runtime load and stress | `npm run ci:runtime-load-smoke`; `npm run ci:runtime-stress` | 32/32 load requests passed at 69.25 ms p95 TTFT; 445-test stress tranche and continuation repetitions passed |
| Documentation/JSON | Docs lint, JSON parse and diff check | See final gate record below |

## Governance Benchmark Evidence

`cargo bench --locked --features bench-support --bench governance_hot_paths`
was rerun for the `8ca79a62` tranche on 2026-07-14 with Criterion's 100-sample
configuration. The current maximum-bound estimate intervals were:

| Maximum-bound case | Criterion estimate interval | Status |
| --- | ---: | --- |
| Inspection result, maximum findings | 27.143-27.238 us | pass |
| PDP, 256 compiled rules | 2.768-2.796 us | pass |
| Governed routing, 64 candidates | 6.290-6.393 us | pass |

The earlier CPU-pinned compatibility comparison against `e308fdf6` passed all
eight disabled-path p95/p99 budgets; the worst p99 delta was +4.4%. The current
deterministic local load smoke passed 32/32 requests at 69.25 ms p95 TTFT.
These results do not claim external provider capacity, multi-replica soak, or
production queue/lock/resource SLOs.

## Architecture and Data Flow

Supported channels enter one authenticated gateway boundary, then execute one
ordered application plan: bounded inspection, monotonic classification, pure
PDP, obligation admission, hard-filtered deterministic routing, attached SPI
adapter dispatch, incremental response enforcement, accounting and durable audit. Four
independent per-tenant ArcSwap authorities publish policy, classification,
provider-registry and routing-score active/LKG snapshots. SQLite is the local
compatibility runtime; PostgreSQL with transaction-local tenant context and RLS
is the enterprise authority; Redis remains rebuildable coordination.

## Material Modules and Schemas

- `prodex-domain`, `prodex-application`, `prodex-provider-spi`,
  `prodex-presidio` and `prodex-authn`: typed inspection, policy, obligations,
  routing and identity contracts.
- `prodex-app`: composition, snapshot refresh, authenticated lifecycle APIs,
  durable session/audit workers, SIEM export, unary/SSE PEPs, and the governed
  Gemini Live compatibility PEP. Virtual-key realtime sessions reserve bounded
  tokens, account each text frame, reconcile terminal usage, and retain one
  provider/profile for the session.
- `prodex-storage`, `prodex-storage-sqlite-runtime` and
  `prodex-storage-postgres-runtime`: generic four-kind governance repository,
  sessions, audit, export and SIEM outbox adapters.
- PostgreSQL/SQLite migrations: immutable artifact revisions, active/LKG
  pointers, approvals/activation history, sessions/revocations, audit-chain
  metadata, SIEM outbox and dead letters. Existing personal SQLite behavior is
  compatible; enterprise/bank modes require PostgreSQL authority.
- OpenAPI and sample artifacts document Policy, ClassificationRules,
  ProviderRegistry and RoutingScores revision/approval/activation/rollback
  lifecycles and canonical classification checksums.

Superseded independent request-policy decisions were removed from adapters;
local and Presidio detectors remain bounded inspection implementations behind
the application contract. Redundant per-candidate selection log writes were
removed in favor of one bounded route-decision trace.

## Rollout, Rollback, and Failure Behavior

Rollout proceeds `personal` to `enterprise_observe`, then tenant-scoped
`enterprise_enforce`; `bank_enforce` is enabled only with private listener,
PostgreSQL, fail-closed inspection, approved snapshots, projected secrets and
durable audit. Rollback activates a prior approved immutable revision and keeps
current LKG/audit history. It never lowers retained classification, revives a
revoked provider/session, expands the eligible set or retries after commit.

Missing identity, mandatory inspection, valid policy/registry, secret reference
or durable audit denies before dispatch in bank mode. PostgreSQL mutation/audit
failure rolls back atomically. SIEM delivery failure retries from the durable
outbox and becomes visible dead-letter state. External inspection/webhook
failure follows the configured bounded observe/enforce failure mode. Provider
transport failure may retry only before response commitment and within the
original eligible set.

## Exact Residual Risks and Boundaries

1. Dynamic routing resolves eligible heterogeneous projected-credential
   adapters and may advance through the immutable eligible fallback set only
   before response commitment. Unsupported/unbound adapters fail unavailable;
   managed regional provider failover remains deployment acceptance evidence.
2. Mandatory governed data-plane audit is submitted to a bounded background
   writer and synchronously acknowledged only after the tenant audit-chain and
   SIEM-outbox transaction commits. This avoids request-thread filesystem/DB
   calls, but intentionally couples dispatch availability and latency to the
   authoritative database and bounded writer queue.
3. Policy-selected Presidio and guardrail-webhook calls execute on the request
   path. They have explicit timeout, no-proxy/no-redirect behavior where
   applicable, response-size bounds and concurrency/admission limits, but remain
   external latency/failure dependencies.
4. PostgreSQL four-kind lifecycle, RLS-scoped session storage, atomic
   audit/outbox writes and leased SIEM delivery passed the disposable live
   backend proof. Managed-database failover and a real external SIEM outage
   exercise remain deployment acceptance work.
5. Durable session state is hydrated by configured authority tenant. Each
   revocation advances the shared authority epoch and replicas poll it every
   250 milliseconds before refreshing cached sessions. The epoch path has a
   focused shared-authority regression; a deployed two-gateway chaos proof is
   not recorded.
6. Hash-targeted revoke remains an authenticated control-plane operation. The
   `current` route also accepts a data-plane virtual key, hashes the raw
   `session_id` header server-side, verifies tenant/principal/scope ownership,
   and atomically revokes only that caller's governed session.
7. Vault protocol authentication and lease renewal are delegated to the
   deployment's Vault Agent, Secrets Store CSI driver or equivalent injector.
   Prodex's production adapter is the bounded projected-secret provider; a
   direct Vault HTTP client is intentionally not another secret authority.
8. Browser Authorization Code plus PKCE S256 is an opt-in gateway-admin flow
   with state/nonce validation, bounded token exchange, secure cookies, and
   logout. PostgreSQL+Redis deployments share one-time transactions and
   sessions across replicas; session lookup revalidates the OIDC token. Signed,
   recent OIDC back-channel logout tokens revoke bounded hashed `sid`/`sub`
   session indexes across replicas.
9. Policy-selected execution approval is request-digest/revision bound,
   quorum-gated, atomically single-use, and exposed through content-free admin
   HTTP. Break-glass approval is a separate, independently approved,
   one-hour-bounded and revocable credential enforced only for bounded,
   mandatorily audited retention purge; it is not a generic bypass.
10. SCIM-linked group and department attributes are cached on matching active
    virtual-key identities and evaluated by the typed PDP. The linkage requires
    one unambiguous same-tenant `user_id` match and fails closed otherwise.
11. Configured `gateway.workload_identity` verifies JWT/JWKS issuer, audience,
    tenant, subject, and scope. With `mtls_required`, the direct Rustls listener
    verifies the client chain and requires JWT `cnf.x5t#S256` binding to the
    peer leaf certificate. Managed issuer/CA rotation remains acceptance work.
12. `npm ci` is not an executable gate in this workspace: no lockfile is
    tracked, and a generated lockfile rejects the root's intentionally
    cross-platform workspace packages on one host. `npm test` is the supported
    repository gate.

## Prior Release and Full-Gate Record

The table below is retained historical evidence from the earlier release
candidate. It is not a release decision or full-gate claim for the current
tranche. Current external deployment, SIEM, managed failover, PKI rotation,
multi-replica acceptance blockers remain open.

| Gate | Status | Final evidence owner |
| --- | --- | --- |
| PostgreSQL governance/RLS integration | passed | live disposable PostgreSQL proof |
| SIEM outbox/exporter integration | passed | SQLite exporter tests and live PostgreSQL outbox lifecycle |
| `cargo fmt --check` | passed | release candidate |
| Locked workspace Clippy, all targets/features, warnings denied | passed | release candidate |
| Full Rust/npm test gate | passed | 2,835 Rust tests/189 suites; `npm test` passed |
| Docs/JSON/architecture/deployment guards as one final run | passed | release candidate |
| Secret/PII tracked-diff scan | passed | pre-commit verifier |
| Fuzz and load/stress gates | passed | 117,401 fuzz executions; 32/32 load smoke; runtime stress pass |
| Multi-replica soak/chaos | not claimed | deployment acceptance owner |
| Governance-disabled compatibility delta | passed | CPU-pinned baseline/candidate comparison |
| Prior release decision | approved | historical npm/GitHub `0.285.0`; not a current-tranche release decision |

## Prior Candidate Commands

The earlier candidate recorded:

```bash
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo test --locked --workspace --all-features
npm test
npm run docs:lint
npm run ci:enterprise-docs-guard
npm run ci:application-boundary-guard
npm run ci:production-boundary-guard
npm run ci:auth-boundary-guard
npm run ci:config-boundary-guard
npm run ci:control-plane-boundary-guard
npm run ci:domain-boundary-guard
npm run ci:observability-boundary-guard
npm run ci:provider-spi-boundary-guard
npm run ci:storage-boundary-guard
npm run ci:storage-postgres-proof
npm run ci:storage-sqlite-boundary-guard
npm run ci:storage-redis-boundary-guard
npm run ci:deployment-security-guard
npm run ci:backup-restore-drill
npm run ci:runtime-load-smoke
npm run ci:runtime-stress
cargo audit
cargo deny check
cargo bench --features bench-support --bench governance_hot_paths
cargo bench --features bench-support --bench runtime_proxy_hot_paths
```

The governance fuzz target ran under nightly for 31 seconds and 117,401
executions. `npm ci` was tested and rejected for the documented lockfile and
cross-platform-workspace reason; it is not substituted for `npm test`.

## Conclusion

The candidate contains the planned governance boundaries and recorded evidence,
with unsupported capabilities and deployment acceptance work stated explicitly.
