# Security Test Matrix

Status meanings:

- `pass`: authoritative current test evidence proves the control;
- `partial`: some control exists, but the stated requirement is not proven end to end;
- `missing`: current code contradicts the requirement or no proof exists;
- `pending`: evidence collection is incomplete.

## Phase 1 Mandatory Controls

| Threat | Required control | Test/evidence | File(s) | Current status |
| --- | --- | --- | --- | --- |
| Remote shell exposed implicitly | tunnel opt-in, loopback-only local server, prominent warning | CLI default/limit tests; `expose_connection_flood_keeps_fixed_worker_count` | `crates/prodex-cli/tests/src/expose.rs`; `crates/prodex-app/src/expose/tests.rs` | pass |
| Capability leaks through URL/logs | one-time fragment bootstrap; header exchange; opaque redacted session | URL, status, `Debug`, and tunnel-error sentinels | `crates/prodex-app/src/expose/tests.rs`; `docs/adr/1068-expose-session-tunnel-model.md` | pass |
| Bootstrap replay or stale sessions | single use, short TTL, rotation/revocation, secure cookie attributes | expiry/replay, cookie, rotation, revoke, and idle tests | `crates/prodex-app/src/expose/tests.rs` | pass |
| Cross-origin shell mutation | strict Origin/Host plus session-bound CSRF policy | missing/foreign Origin, duplicate header, Host, and mutation negatives | `crates/prodex-app/src/expose/tests.rs` | pass |
| Expose resource exhaustion | bounded workers, clients, queues, input rate/body, idle timeout, shutdown | slow-socket flood, queue saturation, max-client, and PTY join tests | `crates/prodex-app/src/expose/tests.rs` | pass |
| Data plane forwards unknown routes | explicit data-plane allowlist; `Unknown` returns stable 404 before backend | server and deployed-backend call-count tests | `crates/prodex-gateway-server/src/tests.rs`; `crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_tests/gateway_application_boundary.rs` | pass |
| Control plane accepts wrong route | explicit control-plane allowlist plus selected health probes | per-plane matrix and unknown-control-operation tests | `crates/prodex-gateway-server/src/tests.rs`; `crates/prodex-gateway-http/tests/http_policy.rs` | pass |
| Front/application parser disagreement | one exact `CanonicalRequestTarget` object used for classification, auth, audit, metrics, and forwarding | shared-parser corpus, source guard, and exact-forwarding tests | `crates/prodex-gateway-server/src/lib.rs`; `crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_application_runtime.rs`; `crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_pipeline.rs` | pass |
| Ambiguous request-target bypass | reject malformed encoding, encoded separators, backslashes, dot/repeated segments, whitespace, absolute form, non-ASCII | raw TCP corpus, 10,000-case property test, fuzz harness | `crates/prodex-gateway-http/tests/http_policy.rs`; `fuzz/fuzz_targets/canonical_request_target.rs` | pass |
| Route alias crosses planes | alias publication validates typed target plane | canonical/versioned/compatibility kind-and-plane pairs | `crates/prodex-gateway-http/src/route.rs`; `crates/prodex-gateway-http/tests/http_policy/routing.rs` | pass for current static aliases |
| OIDC/JWKS SSRF | one validated URL/origin/address/redirect policy in production | malicious document/redirect/IP/NAT64/6to4 and connected-peer tests | `crates/prodex-authn/tests/oidc.rs`; `crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gateway_admin_auth/tests.rs` | pass |
| OIDC network I/O stalls auth | immutable cache snapshot; bounded background refresh/LKG | parsed-JWKS `ArcSwap`, request-path no-I/O, stale/LKG, and shutdown tests | `crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gateway_admin_auth/cache.rs`; `crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gateway_admin_auth/tests.rs` | pass |
| OIDC resource exhaustion | body/time/cache/concurrency/retry bounds | oversized/slow body, cache/resolver caps, backoff tests, fuzz harness | `crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gateway_admin_auth/tests.rs`; `fuzz/fuzz_targets/oidc_endpoint_policy.rs` | pass |
| Runtime configuration changes under active requests | one typed startup snapshot; no tuning reads on hot paths | loader count/error/listener-order, gateway refresh-parity, URL-redaction tests, and hot-path guards | `crates/prodex-app/src/runtime_config`; `crates/prodex-app/src/app_commands/runtime_launch/gateway_config.rs`; `crates/prodex-app/tests/src/runtime_config.rs`; `scripts/ci/config-boundary-guard.mjs` | pass: gateway provider, base URL, provider compatibility, auth fallback, state fallback, and Gemini model settings are captured before secret resolution and binding; projected-secret refresh reuses non-secret state and resolves only credential material |
| Broker secret visible in argv/env | bounded versioned inherited IPC bootstrap | command-plan snapshot and malformed/truncated/oversized bootstrap tests | `crates/prodex-runtime-broker/src/process.rs`; `crates/prodex-runtime-broker/tests/src/process.rs` | pass |
| Broker secret leaks via formatting | redacted wrapper, no raw `Display`, zeroize on drop | formatting, error, log, audit, header, and process-plan sentinels | `crates/prodex-runtime-broker/src/admin.rs`; `crates/prodex-runtime-broker/tests/src/lib.rs` | pass |
| Broker secret persists in registry/backup/health | metadata/secret separation and non-secret health identity | registry/backup/health snapshots, rotation, native Windows gate | `crates/prodex-app/src/runtime_broker/registry/store.rs`; `.github/workflows/ci.yml` | pass on Unix; Windows native gate configured, first CI execution pending |
| Timing oracle in bearer comparison | one constant-time comparison helper | centralized caller inventory and functional tests | `crates/prodex-runtime-broker/src/admin.rs`; `crates/prodex-app/src/runtime_broker/admin.rs` | pass |

The original focused baseline passed 65 boundary cases across ten suites, seven expose tests, and
five gateway OIDC tests, while still asserting insecure compatibility behavior. Phase 1 evidence
above comes from the new contract tests and is separate from that baseline.
Invalid, disallowed-zero, and overflowing gateway OIDC timing environment values now prove
aggregated fail-closed startup rejection; cache-header clamping remains covered only for untrusted
HTTP metadata that can reach a valid typed runtime snapshot.

## Cross-Cutting Acceptance Controls

| Threat | Required proof | Test/evidence | File(s) | Status |
| --- | --- | --- | --- | --- |
| Credential-scope bypass | production canonical authn/application gate for every credential kind | evidence-backed principal/OIDC scope negatives, per-plane route tests, and production guard | `crates/prodex-application/src/request_context.rs`; `crates/prodex-authn/src/evidence.rs`; `crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_tests/gateway_auth_evidence.rs`; `scripts/ci/production-boundary-guard.mjs` | pass: production derives required scope from the canonical route and authenticates verified principal/OIDC evidence through the typed application boundary; compatibility planners are guard-rejected |
| Cross-tenant access | authoritative application authz plus storage-adapter negatives | tenant/scope matrices, deterministic in-transaction foreign-preimage no-write test, and two-proxy file/SQLite foreign-replacement tests | `crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gateway_admin_auth/admin.rs`; `crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gateway_admin_keys.rs`; `crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gateway_admin_scim.rs`; `crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_tests/gateway_admin_toctou.rs`; `scripts/ci/production-boundary-guard.mjs` | pass: application-owned governance scope is rechecked against authoritative key/SCIM pre-images and planned post-images inside each backend lock/transaction before mutation; scope denial is a stable audited 403 with no write |
| Control-plane replay or duplicate mutation | principal-scoped hashed keys and a durable duplicate check before side effects | missing-key, duplicate, restart/backend, and no-secret-persistence tests | `crates/prodex-domain/src/idempotency.rs`; `crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gateway_admin_execution.rs`; `crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gateway_admin_store_mutation/atomic.rs`; `crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_tests/gateway_admin_crud.rs` | pass: duplicates return one stable 409 with no second mutation, audit event, response body, or token stored |
| Concurrent control-plane writes fork audit or partially commit | mutation projection, completed idempotency marker, and canonical audit link share one backend lock/transaction | bounded file history, terminal-head digest, rollback, SQLite transaction, and backend adapter tests | `crates/prodex-domain/src/audit/digest.rs`; `crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gateway_admin_store_mutation/atomic.rs`; `crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gateway_admin_store_mutation/atomic/postgres.rs` | pass: file, SQLite, PostgreSQL, and Redis compute/commit one chain link inside the authoritative atomic boundary |
| Lost key or SCIM update | authoritative `If-Match` check inside the mutation lock/transaction | stale/current/wildcard key and SCIM precondition tests, including PATCH and PUT | `crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gateway_admin_keys.rs`; `crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gateway_admin_scim.rs`; `crates/prodex-gateway-http/tests/http_policy/routing.rs` | pass: stale preconditions produce an audited 412 without mutation |
| Provider secret escapes application boundary | configured `SecretRef` reaches invocation and resolves only in adapter | configured-reference, rotation, redaction, and resolution-failure tests | `crates/prodex-app/src/app_commands/runtime_launch/gateway_secret_config.rs`; `crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_application_data_plane.rs`; `crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_transport/projected_credential.rs` | pass |
| Mid-stream rotation | stream-commit/affinity regressions | HTTP/WebSocket commit-state tests | `crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_response_spend.rs`; `crates/prodex-app/tests/src/runtime_proxy/affinity.rs` | pass |
| Lost accounting on cancellation | partial-stream reconciliation | completed/interrupted/cancelled reconciliation tests | `crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_application_data_plane.rs`; `crates/prodex-storage/tests/reconciliation_lifecycle.rs` | pass |
| Unbounded network-facing work | capacity, timeout, overload, cancellation test for every queue/cache/retry | expose/gateway/OIDC/broker/load bounds | `crates/prodex-app/src/expose/tests.rs`; `tests/load/scenarios.json`; `tests/load/runtime-proxy-load.mjs` | pass for implemented paths; baseline and refactor default stress raw-marker thresholds remain documented failures |
| Request-path schema migration | architecture guard and adapter tests | negative DDL fixture and reconciliation tests | `scripts/ci/production-boundary-guard.mjs`; `crates/prodex-storage/tests/reconciliation_lifecycle.rs` | pass |
| Secret-bearing CLI arg or URL query reintroduced | source guard with malicious self-tests | `ci:secret-boundary-guard` | `scripts/ci/secret-boundary-guard.mjs` | pass |
| Credentials embedded in service URLs | reject userinfo, passwords, queries, and fragments before request, logging, registry, audit, or child-plan construction; redact URL-bearing `Debug` surfaces | quota/env, Super tail/top-level, profile plan, Presidio, runtime log/registry/local-rewrite, webhook, and telemetry sentinels | `crates/prodex-app/src/quota_support.rs`; `crates/prodex-app/src/profile_local_config.rs`; `crates/prodex-app/src/runtime_broker/spawn.rs`; `crates/prodex-app/src/runtime_launch/proxy_startup.rs`; `crates/prodex-cli/src/runtime_args.rs`; `crates/prodex-presidio/src/lib.rs` | pass |
| Unsafe code spreads | workspace guard and platform safety contract | Clippy/allow/size/boundary guards | `Cargo.toml`; `scripts/ci/allow-attribute-guard.mjs`; `crates/prodex-secret-store/src/secure_file` | pass |
| Dependency/supply-chain compromise | locked audit/deny, immutable inputs, SBOM, checksums, provenance, Gitleaks | supply-chain job and self-test guard | `.github/workflows/ci.yml`; `.github/workflows/standalone-release.yml`; `scripts/ci/supply-chain-guard.mjs` | pass |

## Phase 4 Secret-File Controls

| Threat | Required control | Test/evidence | File(s) | Current status |
| --- | --- | --- | --- | --- |
| Final or parent link redirects a secret operation | handle-relative traversal, no-follow opens, reparse rejection, identity checks | Unix link/replacement/refresh-lock tests and native Windows gate | `crates/prodex-secret-store/src/secure_file`; `crates/prodex-secret-store/tests/src/tests.rs`; `.github/workflows/ci.yml` | pass on Unix; Windows native gate configured, first CI execution pending |
| Weak owner, mode, ACL, or parent trust exposes a secret | current-owner `0600`, trusted parents, projected `0440`, private Windows DACL | mode/owner/parent/projected and malicious-group-ACE tests | `crates/prodex-secret-store/src/secure_file/windows.rs`; `crates/prodex-secret-store/tests/src/tests.rs` | pass on Unix; Windows native gate configured, first CI execution pending |
| Partial or oversized secret file is consumed | metadata precheck and bounded read with overflow sentinel | backend/provider/refresh custom-bound tests | `crates/prodex-secret-store/src/file_backend.rs`; `crates/prodex-secret-store/src/projected_provider.rs` | pass |
| Secret write publishes partial or public content | private temporary, flush, atomic replace, directory flush/write-through, identity check | atomic replacement, `0600`, residue, and symlink-target tests | `crates/prodex-secret-store/src/file_backend.rs`; `crates/prodex-secret-store/tests/src/tests.rs` | pass on Unix; Windows native gate configured, first CI execution pending |
| Kubernetes rotation mixes generations | pin and validate one `..data` generation | rotation, anchoring, escape, nested-target, and reparse tests | `crates/prodex-secret-store/src/projected_provider.rs`; `crates/prodex-secret-store/tests/projected_secret_provider.rs` | pass on Unix; Windows native gate configured, first CI execution pending |
| Secret material survives or escapes generic APIs | zeroize-on-drop, no material `Clone`/serde, closure-scoped exposure | compile-fail doctests and zeroize/redaction tests | `crates/prodex-domain/src/secrets.rs`; `crates/prodex-domain/tests/secrets.rs`; `crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_tests/projected_provider.rs` | partial: the core type and production provider adapter are scoped; telemetry/webhook snapshots still retain cloneable raw strings |
| Unsupported keyring appears production-capable | production rejects the metadata-only stub | selection and operation negatives | `crates/prodex-secret-store/src/keyring_backend.rs`; `crates/prodex-secret-store/tests/src/keyring.rs` | pass |

Compatibility note: `SecretMaterial::expose_secret`, its generic serde implementations, and its
value `Clone` were removed; callers use `with_exposed_secret`. `SecretValue` also no longer clones.
The keyring marker remains source-compatible, but production configuration rejects it explicitly.

Windows evidence: `.github/workflows/ci.yml` now runs the secret-store, runtime-broker,
profile-export, and application broker-capability tests natively on `windows-latest` with Rust
1.97.0. The supply-chain guard rejects removal, unlocked commands, fail-open behavior, or missing
suite coverage. This local Linux host cannot execute that native job, so the matrix makes no
Windows runtime-pass claim until its first successful CI run.

## Phase 7 Supply-Chain Evidence

| Control | Test/evidence | File(s) | Status |
| --- | --- | --- | --- |
| Immutable GitHub Actions | full-SHA pins with readable tag comments and guard | `.github/workflows`; `scripts/ci/supply-chain-guard.mjs` | pass |
| Exact Rust/MSRV | manifests, workflows, Docker builder, and components use 1.97.0 | `Cargo.toml`; `rust-toolchain.toml`; `Dockerfile` | pass |
| Immutable container inputs | verified image digests and Docker/Compose checks | `Dockerfile`; `compose.yaml`; `deploy/kubernetes/prodex-gateway.yaml` | pass |
| License and dependency policy | audit plus deny advisories/bans/licenses/sources | `deny.toml`; `.github/workflows/ci.yml` | pass |
| Unused direct dependencies | pinned `cargo-machete 0.9.2 --with-metadata` | `.github/workflows/ci.yml` | pass |
| Release integrity | SPDX SBOM, attestations, and verified `SHA256SUMS` | `.github/workflows/standalone-release.yml` | pass |
| Credential leak scan | digest-pinned Gitleaks and CLI/URL capability guard | `.github/workflows/ci.yml`; `scripts/ci/secret-boundary-guard.mjs` | pass |
| Production secret projection | gateway, control-plane, and migration commands resolve projected `SecretRef` files | `compose.yaml`; `deploy/compose-gateway-policy.toml`; `deploy/kubernetes/prodex-gateway.yaml`; `src/bin/prodex-gateway.rs`; `src/bin/prodex-control-plane.rs`; `src/enterprise_serve.rs` | pass: dedicated data-plane and control-plane policies use typed projected secrets; the control plane has its own non-provider policy and one-replica deployment |

## Characterization Order

1. Add failing Phase 1A expose default/leak/resource tests; then change behavior.
2. Add failing Phase 1B unknown-route and canonical-target tests; then change the shared parser/front.
3. Add Phase 1C malicious endpoint and no-request-I/O tests around one production policy; then wire
   both launch paths to it.
4. Add Phase 1D command-plan/health/registry redaction tests; then replace argv bootstrap with IPC.
5. Run all existing affinity, streaming, accounting, CLI, provider, and boundary suites after every
   coherent slice.

No test may be weakened to match implementation. Internal-detail tests may be replaced only by a
stronger contract test in the same change.
