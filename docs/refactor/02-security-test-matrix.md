# Security Test Matrix

Status meanings:

- `pass`: authoritative current test evidence proves the control;
- `partial`: some control exists, but the stated requirement is not proven end to end;
- `missing`: current code contradicts the requirement or no proof exists;
- `pending`: evidence collection is incomplete.

## Phase 1 Mandatory Controls

| Threat | Required control | Authoritative test/evidence | Current status |
| --- | --- | --- | --- |
| Remote shell exposed implicitly | tunnel opt-in, loopback-only local server, prominent warning | CLI default/limit tests; `expose_connection_flood_keeps_fixed_worker_count` | pass |
| Capability leaks through URL/logs | one-time fragment bootstrap; header exchange; opaque redacted session | expose URL, status, `Debug`, and tunnel-error sentinel tests; ADR 1068 delivery exception | pass |
| Bootstrap replay or stale sessions | single use, short TTL, rotation/revocation, secure cookie attributes | bootstrap expiry/replay, cookie, rotation, revoke, and idle tests | pass |
| Cross-origin shell mutation | strict Origin/Host plus session-bound CSRF policy | missing/foreign Origin, duplicate header, Host, and mutation integration negatives | pass |
| Expose resource exhaustion | bounded workers, clients, queues, input rate/body, idle timeout, shutdown | fixed-worker slow-socket flood, queue saturation, max-client, PTY join tests | pass |
| Data plane forwards unknown routes | explicit data-plane allowlist; `Unknown` returns stable 404 before backend | gateway-server and deployed legacy backend call-count tests | pass |
| Control plane accepts wrong route | explicit control-plane allowlist plus selected health probes | per-plane server matrix and unknown control-operation tests | pass |
| Front/backend parser disagreement | one strict `CanonicalRequestTarget` used for classification, auth, audit, metrics, forwarding | shared backend classifier, exact-forwarding test, 10,000-case property corpus, cargo-fuzz target | pass |
| Ambiguous request-target bypass | reject malformed encoding, encoded separators, backslashes, dot/repeated segments, whitespace, absolute form, non-ASCII | raw TCP negative corpus plus `canonical_request_target` fuzz harness | pass |
| Route alias crosses planes | alias publication validates typed target plane | canonical/versioned/compatibility alias kind-and-plane pairs | pass for current static aliases |
| OIDC/JWKS SSRF | one validated URL/origin/address/redirect policy in production | malicious URL/document/redirect/IP/NAT64/6to4 and pinned connected-peer tests | pass |
| OIDC network I/O stalls auth | immutable cache snapshot; bounded background refresh/LKG | parsed-JWKS `ArcSwap`, post-prefetch env mutation, request-path no-I/O, stale/LKG, and shutdown tests | pass |
| OIDC resource exhaustion | body/time/cache/concurrency/retry bounds | oversized/slow body, 128-key/cache/resolver caps, timeout/backoff tests and OIDC fuzz harness | pass |
| Runtime configuration changes under active requests | one typed startup snapshot; no core proxy/OIDC/broker/provider tuning reads from the request or polling hot path | `runtime_config_reads_each_environment_key_once`, `runtime_config_aggregates_errors_without_values`, `runtime_config_failure_precedes_listener_bind`, post-start OIDC env-mutation tests, Gemini hot-path config guard, and app Clippy with warnings denied | pass; remaining `cwd`, temp-dir, user-shell, and external CLI discovery reads are request inputs or discovery, not mutable tuning |
| Broker secret visible in argv/env | bounded versioned inherited IPC bootstrap | exact command-plan snapshot, hidden CLI rejection, malformed/truncated/oversized bootstrap tests | pass |
| Broker secret leaks via formatting | redacted secret wrapper, no raw `Display`, zeroize on drop | `Debug`, `Display`, capability-error, log, audit, header-sensitivity, and process-plan sentinels | pass |
| Broker secret persists in registry/backup/health | metadata/secret separation and non-secret health identity | exact registry-backup and health payload snapshots, legacy-field negatives, and instance-bound capability rotation tests | pass on Unix; Windows broker capability-file DACL hardening and runtime evidence remain pending |
| Timing oracle in bearer comparison | one constant-time comparison helper | centralized caller inventory plus admin-auth/cleanup functional tests | pass |

The original focused baseline passed 65 boundary cases across ten suites, seven expose tests, and
five gateway OIDC tests, while still asserting insecure compatibility behavior. Phase 1 evidence
above comes from the new contract tests and is separate from that baseline.

## Cross-Cutting Acceptance Controls

| Threat | Required proof | Status |
| --- | --- | --- |
| Credential-scope bypass | production application/authn gate, legacy differential matrix, negative auth tests for every data/control route and credential kind | pass; production data/control routing uses canonical principals and typed operation scopes, with anonymous behavior isolated as an explicit compatibility principal |
| Cross-tenant access | authz plus storage-adapter negative tests | pass; tenant-bound data/control admission and SQLite/Postgres reconciliation reject mismatched ownership |
| Mid-stream rotation | stream-commit/affinity regressions | pass; commit-state retry planner forbids provider rotation after first byte or cancellation and focused HTTP/WebSocket affinity tests pass |
| Lost accounting on cancellation | partial-stream reconciliation tests | pass; production stream exits classify completed, interrupted, and cancelled outcomes, commit observed usage, and release the remainder through the application reconciliation port |
| Unbounded network-facing work | capacity, timeout, overload, cancellation test for each queue/cache/retry | pass for exposed shell, gateway admission, OIDC caches/refresh, broker bootstrap, provider attempts, and background queues; the default historical stress scenario still exceeds its raw diagnostic-marker threshold on both the baseline and refactor and is reported separately |
| Request-path schema migration | architecture guard and adapter tests | pass; the production-boundary guard rejects migration/DDL fixtures and storage reconciliation tests keep schema work outside request paths |
| Secret-bearing CLI arg or URL query reintroduced | `ci:secret-boundary-guard` scans production Rust/CLI/docs plus fixture regions and rejects malicious self-tests | pass |
| Unsafe code spreads | workspace guard and platform-module safety contract | pass; workspace `unsafe_op_in_unsafe_fn` denial and allow/size/boundary guards pass, while platform-specific unsafe calls remain isolated with per-call safety contracts |
| Dependency/supply-chain compromise | locked builds, full audit/deny policy, immutable actions/images, SBOM/checksums/provenance verification, Gitleaks | pass |

## Phase 4 Secret-File Controls

| Threat | Required control | Authoritative test/evidence | Current status |
| --- | --- | --- | --- |
| Final or parent link redirects a secret operation | handle-relative traversal, no-follow final opens, reparse rejection, and handle identity before replace/remove | Unix final/parent symlink, replacement, and refresh-lock identity regressions; cfg(windows) reparse tests; Windows target check and all-target Clippy | pass on Unix; Windows compile-verified, runtime evidence pending because the MinGW linker tools were unavailable |
| Weak owner, mode, ACL, or parent trust exposes a secret | current-owner `0600` files; trusted non-writable parents; projected owner/root plus only `0440`-compatible group read; private Windows DACL | Unix owner/mode, group-readable/group-writable, parent-mode, projected-mode tests; Windows DACL unit contract; Windows target check and all-target Clippy | pass on Unix; Windows compile-verified, runtime evidence pending because the MinGW linker tools were unavailable |
| Partial or oversized secret file is consumed | metadata precheck plus bounded handle read with a one-byte overflow sentinel | file backend, projected provider, and refresh-result oversized tests | pass |
| Secret write publishes partial or public content | same-directory private temporary, flush, atomic replace, directory flush/Windows write-through, and post-replace identity check | atomic inode replacement, `0600`, no-temp-residue, and symlink-target preservation tests; Windows target check and all-target Clippy | pass on Unix; Windows compile-verified, runtime evidence pending because the MinGW linker tools were unavailable |
| Kubernetes rotation mixes generations | `..data` is the sole intentional link, its target is one normal component, and value/version read through one pinned generation-directory handle | atomic projection rotation, generation anchoring, escape, nested-target, and cfg(windows) controlled-reparse tests; Windows target check and all-target Clippy | pass on Unix; Windows compile-verified, runtime evidence pending because the MinGW linker tools were unavailable |
| Secret material survives or escapes generic APIs | zeroize-on-drop owners, no material `Clone` or serde, redacted formatting, and closure-scoped byte exposure | `SecretMaterial` compile-fail doctests, zeroize trait checks, redaction tests, and migrated resolver/provider call sites | pass |
| Unsupported keyring appears production-capable | production configuration rejects the explicitly unsupported metadata-only stub, whose operations also fail unsupported | application selection and secret-store operation negatives | pass |

Compatibility note: `SecretMaterial::expose_secret`, its generic serde implementations, and its
value `Clone` were removed; callers use `with_exposed_secret`. `SecretValue` also no longer clones.
The keyring marker remains source-compatible, but production configuration rejects it explicitly.

Windows evidence: Rust 1.97.0's `x86_64-pc-windows-gnu` target passes locked,
all-feature `cargo check` and all-target Clippy. Test linking could not start because
`x86_64-w64-mingw32-dlltool` is not installed, so this matrix makes no Windows runtime-pass claim.

## Phase 7 Supply-Chain Evidence

| Control | Authoritative test/evidence | Status |
| --- | --- | --- |
| Immutable GitHub Actions | third-party `uses:` sites pinned to verified 40-character upstream commits with tag comments; `ci:supply-chain-guard` | pass |
| Exact Rust/MSRV | root/workspace `rust-version`, `rust-toolchain.toml`, every CI setup, and Docker builder use 1.97.0; clippy/rustfmt components installed | pass |
| Immutable container inputs | Rust, Debian, PostgreSQL, Redis, Syft, and Gitleaks use verified registry manifest-list digests; Kubernetes keeps its release image immutable by digest; Docker/Compose guards and `docker build --check` | pass |
| License and dependency policy | `cargo deny check advisories bans licenses sources`; exact-version duplicate exceptions with current transitive owners | pass |
| Unused direct dependencies | pinned `cargo-machete 0.9.2 --with-metadata`; root `postgres` and `prodex-cli` `prodex_shared_types` removed | pass |
| Release integrity | SPDX JSON SBOM and binaries attested; release job verifies attestations, generates `SHA256SUMS`, and verifies it before publishing | pass |
| Credential leak scan | digest-pinned Gitleaks job plus self-testing CLI/URL capability guard | pass |
| Production secret projection | gateway resolves Kubernetes `SecretRef` files under `/run/secrets/prodex` | partial: projected-provider lifecycle boundary passes; migration remains `--url-env` and the zero-replica control plane remains a placeholder pending typed-secret adapter work |

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
