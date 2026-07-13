# Final Enterprise Governance Implementation Report

Date: 2026-07-13 (Asia/Jakarta)

Baseline revision: `e308fdf6`

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
| 3. PDP, lifecycle, audit and SIEM | Pure PDP, four-kind immutable governance lifecycle, maker-checker approval, mandatory durable decision audit, bounded audit export and durable outbox/exporter ports are implemented | policy/approval domain tests; gateway lifecycle regressions; SQLite lifecycle/export tests; live PostgreSQL governance and SIEM outbox tests |
| 4. Governed provider routing | Tenant-aware hard eligibility, deterministic fixed-point score, affinity/revocation and audited dispatch are implemented for the attached adapter | `crates/prodex-provider-spi/tests/governed_routing.rs`; production/provider-SPI boundary guards |
| 5. Unified gateway and bank hardening | Typed identity evidence, edge security, durable governed session admission/revocation, bank startup/runtime validation and deployment guards are implemented | auth/session tests; hash/current-session revoke and audit-export HTTP lifecycle regression; `bank_governance_deployment_matrix_fails_closed`; live PostgreSQL/RLS proof and restore drill |

## Focused Verification Evidence

| Area | Evidence | Result |
| --- | --- | --- |
| Application inspection | `cargo test -q -p prodex-application --test governance_inspection` | 5 passed |
| Application obligations/pipeline | `crates/prodex-application/tests/governance_obligations.rs`, `governance_pipeline.rs` | Focused suite passed in candidate verification |
| Governed routing | `crates/prodex-provider-spi/tests/governed_routing.rs` | Hard filters, fixed-point scoring, fallback, affinity, revocation, bounds and redacted debug covered |
| Runtime policy/bank config | `bank_governance_deployment_matrix_fails_closed`; enforcing snapshot/mode validation tests | Focused suite passed in candidate verification |
| Governance SQLite lifecycle/session/audit/export | `cargo test -q -p prodex-storage-sqlite-runtime --test governance_repository -- --test-threads=1` | 9 passed |
| Governance PostgreSQL lifecycle/session/audit/export | `npm run ci:storage-postgres-proof`; `postgres_policy_governance_activates_and_replays_idempotently`; `postgres_governance_lifecycle_supports_all_artifact_kinds` | live Docker PostgreSQL, RLS, TLS and four runtime tests passed |
| Governance restore | `npm run ci:backup-restore-drill` | pass; measured RPO 1.65 s, RTO 1.243 s, restored tenant/governance fingerprints and audit-chain links intact |
| Gateway lifecycle/session/export | `gateway_policy_http_enforces_maker_checker_replay_cas_tenant_and_lkg`; `gateway_governance_artifacts_use_generic_maker_checker_lifecycle` | Focused HTTP regressions passed |
| Tool metadata bound | `requested_tool_metadata_is_bounded` | passed |
| Webhook content safety | `guardrail_webhook_policy_reason_is_stable_and_content_free` | passed |
| Architecture/security guards | production, application, auth, config, provider-SPI, deployment-security and observability boundary guards | passed in focused verification |
| Documentation/JSON | Docs lint, JSON parse and diff check | See final gate record below |

## Governance Benchmark Evidence

Recorded on 2026-07-13 from the candidate based on `e308fdf6`, using Linux
6.17, an AMD Ryzen 5 PRO 4650G (6 cores/12 threads), and 30 GiB RAM. Criterion
used 100 per-iteration samples. The timed run used 109% CPU and 24,200 KiB peak
RSS.

| Maximum-bound case | p50 | p95 | p99 | p50 throughput | Status |
| --- | ---: | ---: | ---: | ---: | --- |
| Inspection result, maximum findings | 26.926 us | 27.755 us | 28.221 us | 37,139/s | pass |
| PDP, 256 compiled rules | 1.963 us | 2.043 us | 2.066 us | 509,422/s | pass |
| Governed routing, 64 candidates | 5.734 us | 6.069 us | 6.530 us | 174,411/s | pass |

The CPU-pinned compatibility comparison passed all eight disabled-path p95/p99
budgets; the worst p99 delta was +4.4%. The deterministic local load smoke
passed 32/32 requests at 68.56 ms p95 TTFT. These results do not claim external
provider capacity, multi-replica soak, or production queue/lock/resource SLOs.

## Architecture and Data Flow

Supported channels enter one authenticated gateway boundary, then execute one
ordered application plan: bounded inspection, monotonic classification, pure
PDP, obligation admission, hard-filtered deterministic routing, attached SPI
dispatch, incremental response enforcement, accounting and durable audit. Four
independent per-tenant ArcSwap authorities publish policy, classification,
provider-registry and routing-score active/LKG snapshots. SQLite is the local
compatibility runtime; PostgreSQL with transaction-local tenant context and RLS
is the enterprise authority; Redis remains rebuildable coordination.

## Material Modules and Schemas

- `prodex-domain`, `prodex-application`, `prodex-provider-spi`,
  `prodex-presidio` and `prodex-authn`: typed inspection, policy, obligations,
  routing and identity contracts.
- `prodex-app`: composition, snapshot refresh, authenticated lifecycle APIs,
  durable session/audit workers, SIEM export and unary/SSE/WebSocket/Gemini
  PEPs.
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

1. One local rewrite process owns one attached executable provider adapter,
   upstream configuration and credential family. Simultaneous heterogeneous
   adapter dispatch and cross-provider fallback are unsupported. A route naming
   another provider must fail unavailable before dispatch.
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
5. Durable session state is hydrated by configured authority tenant and
   periodically refreshed. Local revocation is immediate, while another
   replica may retain a revoked session until the next five-second refresh;
   cross-replica invalidation/pub-sub and a two-gateway chaos proof are not
   implemented.
6. Both hash-targeted and current-session revoke routes are authenticated
   control-plane routes. The current route hashes the raw `session_id` header
   server-side; a separate ordinary data-plane self-service logout route is not
   implemented.
7. Vault protocol authentication and lease renewal are delegated to the
   deployment's Vault Agent, Secrets Store CSI driver or equivalent injector.
   Prodex's production adapter is the bounded projected-secret provider; a
   direct Vault HTTP client is intentionally not another secret authority.
8. Browser Authorization Code plus PKCE is not a supported gateway-admin flow;
   browser-originated administration remains disabled. Device/service identity
   paths are the supported authenticated surfaces.
9. Execution approval and governance break-glass activation are disabled. The
   repository retains bounded domain primitives and ADRs, but no authoritative
   production token-consumption path is claimed.
10. Group/department ABAC attributes are not supplied by the current identity
    evidence contract. Tenant, principal, role, project, classification,
    provider and revision attributes are enforced.
11. `npm ci` is not an executable gate in this workspace: no lockfile is
    tracked, and a generated lockfile rejects the root's intentionally
    cross-platform workspace packages on one host. `npm test` is the supported
    repository gate.

## Release and Full-Gate Record

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
| Release decision | approved | publish npm/GitHub `0.285.0`; crates.io excluded |

## Commands Executed

The final candidate passed:

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
