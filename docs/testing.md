# Testing

Prodex test speed should come from process-level sharding first, not from making every Rust test process run with high harness concurrency.

## Strategy

- Fast lanes should split safe suites across independent CI jobs or processes.
- Serial lanes should contain tests that mutate process-global state, depend on runtime lifecycle, or share local runtime resources.
- Runtime proxy tests should prefer manifest-owned shards so fragile cases are easy to quarantine without hiding them from CI.
- The runtime manifest uses explicit category tags for `runtime:parallel-safe`, `runtime:serial`, `runtime:stress`, `runtime:env`, and `runtime:quarantine`.
- `npm run ci:runtime-manifest` must fail when a `main_internal_tests::runtime_proxy_` test is neither covered by a manifest case nor intentionally covered by a broad runtime proxy CI shard filter.
- `npm run ci:runtime-manifest` must also fail when broad runtime shard labels or filters drift from the `main-internal-runtime-proxy` workflow matrix.
- Full serial coverage should remain available as a scheduled or manual safety net.

Independent process shards are preferred because each process can own its environment variables, temp homes, runtime log directory, artifacts, and background tasks. Inside risky runtime or global-env shards, keep Rust harness scheduling serial with `--test-threads=1`.

## CI Impact Gating

The CI workflow has a `changes` job that runs `node scripts/ci/ci-impact.mjs --base ... --head ... --github-output` with full git history. Its `heavy` output gates expensive Rust/runtime jobs such as supply-chain checks, npm package smoke, auto-rotate, internal Rust shards, runtime proxy shards, runtime benchmark smoke, and runtime stress.

When the classifier returns `heavy=false`, cheap and broadly relevant checks still run: formatting, docs lint, secret scan, process guards, and release sync. Use this path for docs-only or similarly low-impact changes once the classifier recognizes them.

To force full CI for a change, touch a Rust or workflow path that the classifier marks heavy, or update the classifier rules so the affected path returns `heavy=true`. Pull requests and pushes both use explicit base/head SHAs; first-push events with no usable `before` SHA fall back to the parent of `github.sha`.

## Parallel Safety

Treat a test as parallel-safe only when it avoids process-global state, fixed shared paths, fixed ports, shared profile homes, shared broker state, and long-lived background work that can outlive the test.

Treat a test as serial or quarantined when it touches any of these:

- `std::env` mutation, including model/provider overrides
- current working directory changes
- default runtime log paths, including the OS temp directory pointer such as `/tmp/prodex-runtime-latest.path` on Linux
- shared `.prodex`, `CODEX_HOME`, or `CLAUDE_CONFIG_DIR` state
- policy files, secret backends, or active-profile state visible outside the test tempdir
- runtime broker lifecycle, fixed ports, websocket state, or long-lived proxy tasks
- continuation and affinity state such as `previous_response_id`, `x-codex-turn-state`, or `session_id`

If a runtime test needs parallel coverage, prefer a separate process with isolated temp homes, isolated `PRODEX_RUNTIME_LOG_DIR`, isolated ports, and a bounded timeout. Do not rely on in-process test ordering to protect global state.

## Commands

Current CI building blocks include:

```bash
npm test
npm run test:fast
npm run test:fast:timings
npm run test:serial
npm run test:serial:timings
npm run ci:preflight
npm run ci:impact
npm run ci:release-hygiene
npm run ci:release-metadata-guard
npm run ci:generated-metadata-clean
npm run ci:size-guard-fixtures
npm run ci:super-wildcard-guard
npm run ci:release-cut-fixtures
npm run ci:runtime-hotpath-guard
npm run ci:crate-boundary
npm run ci:enterprise-docs-guard
npm run ci:enterprise-id-boundary-guard
npm run ci:enterprise-binaries-guard
npm run ci:application-boundary-guard
npm run ci:auth-boundary-guard
npm run ci:gateway-security-smoke
npm run ci:config-boundary-guard
npm run ci:control-plane-boundary-guard
npm run ci:observability-boundary-guard
npm run ci:provider-spi-boundary-guard
npm run ci:storage-boundary-guard
npm run ci:storage-postgres-proof
npm run ci:storage-postgres-boundary-guard
npm run ci:storage-redis-boundary-guard
npm run ci:storage-sqlite-boundary-guard
npm run ci:gateway-core-boundary-guard
npm run ci:gateway-http-boundary-guard
npm run ci:deployment-security-guard
npm run ci:churn-hygiene
npm run release:run -- --version 0.x.y --dry-run
npm run release:prepare
npm run changelog:check
npm run docs:lint
npm run ci:runtime-manifest
npm run test:gemini-schema
npm run test:gemini-live
npm run test:runtime-smoke
npm run compat:offline-gate
npm run compat:check
npm run compat:watch
npm run compat:watch-fixtures
npm run compat:watch-ci
npm run compat:capture -- --input /path/to/capture.jsonl --name codex_live_sample
node scripts/ci/runtime-proxy-shard.mjs
npm run ci:runtime-stress -- --suite stress
npm run ci:runtime-stress -- --suite serialized
npm run ci:runtime-stress -- --suite continuation
npm run ci:runtime-load-smoke
node scripts/ci/runtime-env-parallel.mjs --runs 2 --test-threads 4
cargo test -q --workspace --all-features -- --test-threads=1
```

`npm run ci:storage-postgres-proof` is the repeatable local helper for the
database-backed storage race proof. It first verifies its branch-selection
logic without starting Docker, then uses `PRODEX_TEST_POSTGRES_URL` when
provided, or starts a temporary local Postgres container when Docker and `psql`
are available.
Add `-- --storage-postgres-proof` to `npm run ci:preflight` (or set
`PRODEX_PREFLIGHT_STORAGE_POSTGRES_PROOF=1`) when you want that proof included
in the standard local preflight run.

Use `npm run test:fast -- --jobs 4` for local safe lanes that can run as independent child processes. Use `npm run test:serial -- --suite all` for global-env, runtime, continuation, and quarantine lanes that must stay serialized with `--test-threads=1`.

`npm test` is a convenience alias for `npm run test:fast`. It runs the fast lane, not full serial coverage.

Use `npm run test:fast:timings` or `npm run test:serial:timings -- --suite runtime` when tuning local or CI shard balance. The timing mode preserves the existing process split and `--test-threads=1` safety, then prints the slowest completed steps at the end of each runner phase. Add `-- --timings-json` to either runner when you need a single-line JSON payload that can be pasted into notes or compared across CI runs. Use `-- --timings-limit <n>` to show more or fewer slow steps.

Use `npm run test:timing-balance` to summarize one or more saved `timings-json` lines or raw timing JSON arrays:

```bash
npm run test:serial:timings -- --suite stress --timings-json | npm run test:timing-balance
npm run test:timing-balance -- --file /tmp/fast.log --file /tmp/serial.log --limit 20
```

The default output ranks labels by average runtime and includes suggested integer second weights. Use `-- --weights-json` for generic rebalance input shaped as `[{ label, weightSeconds, ... }]`. Use `-- --runtime-stress-hints` when the input labels come from `serial:stress:<test>` or `serial:continuation:<iteration>:<test>`; it emits a `RUNTIME_STRESS_WEIGHT_HINTS` snippet compatible with the weighted runtime stress shard planner in `scripts/ci/runtime-stress.mjs`.

Use `npm run test:runtime-smoke` for a small local runtime invariant suite before broad runtime work. It runs curated log JSON/marker, header preservation, selection affinity, stale continuation, websocket local pressure, and tuning snapshot checks from the shared runtime manifest without changing the broad runtime or stress suites.

Use `npm run test:gemini-schema` after changing Gemini request, response, SSE, tool-schema, or Gemini Live translation code. It is offline and guards the Codex-to-Gemini schema snippets, response metadata mapping, SSE event surface, and Live event vocabulary.

Use `PRODEX_LIVE_GEMINI=1 npm run test:gemini-live` only on machines with configured Gemini credentials. The default path sends one exact-response smoke request. Add `PRODEX_LIVE_GEMINI_EXTENDED=1` to cover exact shell output, file edits, `apply_patch`, reference-repo clone/inspection, optional-tool update discipline, semantic compact, and explicit `exec resume`; add `PRODEX_LIVE_GEMINI_MCP=1` or `PRODEX_LIVE_GEMINI_MULTIMODAL=1` when that environment should also exercise MCP or image-input paths. The live smoke compares the final Codex agent message exactly, so Gemini narration or bridge leakage fails the gate.

Use `npm run ci:runtime-load-smoke` for the bounded CI/scheduled load smoke. It reuses the load harness in mock-only baseline mode with low request count, zero tolerated request errors, zero admission pressure, and a strict p95 TTFT threshold. For local end-to-end proxy coverage after `cargo build`, run `npm run ci:runtime-load-smoke -- --mode proxy --prodex ./target/debug/prodex`; proxy mode relaxes the TTFT threshold enough to avoid machine-specific flakes while still checking error and admission gates.

## Manual Runtime Load

Use the load harness under [tests/load](../tests/load) for local runtime proxy load and stress checks. It is intentionally manual and not part of CI. The harness has a ChatGPT-like mock upstream, a load driver, and scenario defaults for baseline, stress, spike, and soak runs.

Baseline mock-only smoke:

```bash
npm run load:runtime-proxy -- --scenario baseline --start-mock
```

End-to-end proxy run against a temporary `PRODEX_HOME` and local mock upstream:

```bash
cargo build
npm run load:runtime-proxy -- --scenario baseline --start-mock --start-proxy --prodex ./target/debug/prodex
```

Higher pressure examples:

```bash
npm run load:runtime-proxy -- --scenario stress --start-mock --start-proxy --prodex ./target/debug/prodex --profiles 4
npm run load:runtime-proxy -- --scenario spike --start-mock --start-proxy --prodex ./target/debug/prodex --profiles 4
npm run load:runtime-proxy -- --scenario soak --start-mock --start-proxy --prodex ./target/debug/prodex --profiles 4
```

Default thresholds focus on request error rate, p95 time to first byte, and admission pressure. Admission pressure counts local overload-style responses plus runtime log markers such as `runtime_proxy_active_limit_reached`, `runtime_proxy_lane_limit_reached`, `runtime_proxy_queue_overloaded`, `runtime_proxy_overload_backoff`, `profile_inflight_saturated`, and `precommit_budget_exhausted`. Override thresholds when intentionally testing a smaller local cap:

```bash
npm run load:runtime-proxy -- --scenario stress --start-mock --start-proxy --prodex ./target/debug/prodex --max-error-rate 0.05 --max-ttft-p95-ms 2500 --max-admission-pressure-rate 0.10
```

To exercise an already-running proxy, pass the Codex-facing base URL:

```bash
npm run load:runtime-proxy -- --scenario spike --target http://127.0.0.1:9901/backend-api --runtime-log-dir /tmp/prodex-runtime
```

Use `npm run ci:preflight` before pushing broad runtime or release-adjacent changes. It runs release hygiene, size, allow/super-wildcard, crate-boundary, runtime hot-path, churn hygiene, manifest-owned generated metadata sync, runtime-manifest/fmt/cargo-check guards, clippy with warnings denied, and the fast test lane. Churn hygiene fails on actionable violations by default; use `npm run ci:preflight -- --churn-report-only` or `PRODEX_PREFLIGHT_CHURN_REPORT_ONLY=1 npm run ci:preflight` only for exploratory local runs. Add `-- --serial` when a change needs the serialized runtime/global-state lane too, or `-- --dry-run` to inspect the command plan. For local historical audits only, `-- --churn-range old..HEAD --churn-ignore-before latest-tag` keeps churn enforcement focused on commits after the newest version tag inside the selected range.

Use `npm run ci:release-hygiene` for the full release gate. By default it checks `HEAD`; use `-- --range main..HEAD` or `-- --base origin/main --head HEAD` for branch ranges. The runner executes changelog-noise, metadata-only, version metadata, empty release commit, duplicate release version, tag/changelog, release-run/changelog tests, release guard fixtures, and release-cut backcompat fixtures as one serial gate. Changelog generation is user-facing: internal `test`, `ci`, `refactor`, and chore-only commits are omitted. Any `CHANGELOG.md` edit outside a release-like commit is rejected, even when mixed with code or changelog tooling changes; let `npm run release:run -- --version <semver>` render the final `CHANGELOG.md` section in the release commit instead. Real fixes to changelog tooling, for example `fix(changelog): ...` commits that change `scripts/npm/changelog.mjs`, remain valid when they do not edit generated changelog output.

Use `npm run ci:release-metadata-guard` for the focused metadata-only check. By default it checks `HEAD`; use `-- --range main..HEAD` for a branch range, or `-- --staged --assume-release` before committing a release bump. The guard fails only when a release-like commit changes both version metadata such as `Cargo.toml`, `Cargo.lock`, npm package manifests, or versioned install snippets in `README.md`/`QUICKSTART.md`, and non-metadata files.

Use `npm run ci:runtime-hotpath-guard` after proxy hot-path work. It strips `#[cfg(test)]` Rust items, scans runtime proxy hot-path targets for blocking `std::fs`/`fs::` disk operations, file opens, `spawn_blocking`, and OS `thread::spawn`, skips local rewrite test-fixture modules during the default production scan, then permits only narrow allowlisted existing cases with rationale.

Use `npm run ci:crate-boundary` after adding workspace dependencies. It runs the guard self-test, then parses Cargo manifests and fails on direct dependency edges from focused/helper crates into app orchestration, terminal rendering, or runtime-proxy-incompatible layers.

Use `npm run ci:size-guard` after Rust module reshaping. Besides per-file line caps, it fails when one production directory accumulates too many near-limit sibling modules and when the global near-limit Rust file budget grows beyond the checked-in ratchet. Lower `PRODEX_SIZE_GUARD_NEAR_LIMIT_FILES` or `--near-limit-files` after successful splits; the goal is domain ownership, not threshold-only file splits. Use `npm run ci:size-guard-fixtures` after changing the guard itself.

Use `PRODEX_RUNTIME_PROXY_BENCH_CHECK=1 PRODEX_RUNTIME_PROXY_BENCH_THRESHOLD_FILE=scripts/ci/runtime-proxy-bench-thresholds.json cargo bench --locked --features bench-support --bench runtime_proxy_hot_paths` after benchmark-sensitive proxy changes. Checked-in runtime proxy bench scales use the policy encoded in `scripts/ci/runtime-proxy-bench-thresholds.json`: calibrate against the max observed median, keep a 25% nanosecond margin, require at least 15 scale points above observed required scale, avoid per-case scales below 50%, and suppress suggestions smaller than 5 scale points. `node scripts/ci/benchmark-calibration.mjs <bench-log>` applies the same defaults so local or pasted CI logs do not silently tighten tiny benchmark cases to machine-specific values.

Use `npm run ci:churn-hygiene` for a lightweight churn gate. It fails on actionable violations by default and checks `HEAD~1..HEAD` when available. Use `-- --report-only` or `PRODEX_CHURN_HYGIENE_REPORT_ONLY=1 npm run ci:churn-hygiene` only for exploratory local reports, or pass `-- --range main..HEAD`, `-- --staged`, custom `--max-files`, `--max-lines`, `--max-behavior-files`, or `--max-file-lines` values for stricter local review. Historical release/tag ranges may include already-reviewed broad churn; for those local audits, use `-- --range old..HEAD --ignore-before latest-tag` to report the original range while checking only changes after the newest version tag inside the selected range. A pinned reviewed baseline is still accepted when a specific non-release review point is intentional. Baseline ignores are accepted only with explicit `--range` audits; `--base`/`--head` PR and push guard ranges reject them so env options cannot silently shorten the enforced range. Release metadata-only sweeps across Cargo manifests, npm package manifests, versioned install snippets, or changelog files may exceed the file-count threshold, but line-count, largest-file, behavior-file, and subject checks still apply.

Large structural extraction is allowed when the commit has a clear mechanical `refactor`, `test`, `chore`, or `ci` split/move/reshape subject, or when the message declares `Mechanical-only: yes` or `[mechanical-only]`. For staged/worktree checks before committing, pass `-- --message-file .git/COMMIT_EDITMSG` or `-- --message "refactor: split foo"` when validating a prepared message. If behavior changes are needed, put them in a separate smaller commit after the mechanical extraction.

Use `npm run release:run -- --version <semver>` as the official release path. It can bump/sync/test/commit/push/watch CI/trigger publish/watch publish/verify in order, stores resume state under `target/release-run/state.json` by default, supports `-- --resume`, `-- --from <step>`, `-- --to <step>`, and `-- --only <steps>`, and never runs `npm publish` locally; publishing is triggered through `.github/workflows/npm-publish.yml`. Its sync/test path renders the final changelog with `--release-version` and validates generated release metadata before the release commit. Do not run `npm run changelog` after each small change just to commit updated notes; push-facing changelog checks intentionally defer generated changelog drift for non-release commits. Use `-- --dry-run` before mutating a real release.

Use `npm run ci:release-cut-fixtures` after changing low-level release-cut compatibility. `release:cut` remains as a deterministic backcompat helper covered by fixtures; normal human releases should use `release:run`.

Use `npm run release:prepare` before release-adjacent work that should not commit or tag. It checks version/doc sync, available lockfile consistency, generated changelog freshness, docs lint, upstream compatibility baseline, runtime manifest, cargo fmt, and full Rust test binary compilation without publishing.
The default test-compile guard runs `cargo test --locked --workspace --all-targets --all-features --no-run` so workspace lib, bin, integration test, example, and benchmark targets stay compile-covered. Use `npm run release:prepare -- --no-cargo-test` to skip test binary compilation and run `cargo check --locked --workspace --all-targets --all-features` instead. Use `-- --release-version <semver>` only inside release automation that has already rendered the pending changelog as a final release section.

Use `npm run ci:runtime-manifest` after adding or renaming runtime proxy tests. New `main_internal_tests::runtime_proxy_` tests should normally get a targeted `RUNTIME_CI_TEST_CASES` entry; only add or rely on `RUNTIME_CI_BROAD_SHARD_FILTERS` when a broad CI shard intentionally owns that whole module or prefix. Broad shard filters must mirror the `label|filter` entries in the `main-internal-runtime-proxy` matrix in `.github/workflows/ci.yml` and must not match tests outside runtime CI ownership.

When changing `prodex-context` audit, prose compression, or command-output context-saver helpers, run `cargo test -q -p prodex-context`. If the `prodex context compact-output` CLI surface changes, also run a focused CLI parse/handler test. The command-output helpers are pure and opt-in; they should not require runtime proxy tests unless a separate runtime integration changes.

When changing `prodex info` runtime tuning output, run the focused `cargo test -q runtime_tuning_snapshot_reports_effective_policy_and_env_values -- --test-threads=1` check so env, policy, and default-derived values stay aligned.

Use `npm run compat:check` before changing runtime proxy assumptions. It is offline and verifies that `scripts/compat/upstream-baseline.json` still records the critical upstream Codex files, Responses/compact routes, SSE/websocket stream events, and headers that Prodex preserves or replaces. Baseline format version 2 also records semantic check groups that tie route, header, event, and co-occurrence expectations back to specific upstream files instead of relying only on flat `required_contains` tokens.

Use `npm run compat:offline-gate` before changing runtime proxy transport, preserved/replaced headers, SSE handling, websocket handling, Responses route handling, compact route handling, or replay fixture tooling. It runs the offline upstream baseline guard plus the scrubbed replay fixture checks, and `.github/workflows/ci.yml` requires the same gate in the `compat-replay-gate` job without network access.

Use `npm run compat:watch` when network access is available. It fetches current upstream Codex critical files from the latest release tag, falls back to `main` only when the tag raw file is unavailable, and reports missing required tokens or semantic groups as upstream drift.

Use `npm run compat:watch-ci` to run the same artifact-producing wrapper used by `.github/workflows/upstream-compat.yml`. It writes `report.json`, text logs, metadata, and `summary.md` under `RUNNER_TEMP/upstream-compat` or `target/upstream-compat`.

Use `npm run compat:watch-fixtures` after changing upstream watch logic or fixtures. It runs the offline fixture regression tests for the upstream watch tool. Release hygiene fixtures are synthetic temp git repos, so they no longer depend on preserving specific historical bad commits.

Use `npm run compat:capture -- --input capture.jsonl --name codex_live_sample` to convert offline captured Codex or Claude traffic into scrubbed replay fixtures under `crates/prodex-app/tests/fixtures/compat_replay`. The tool does not capture traffic and never uses the network; it only normalizes local JSON, JSONL, or text input into a deterministic fixture.

Compat capture input should usually be JSONL, one record per line. Supported record `type` values are `request`, `response`, `event`, `websocket_message`, and `sse_stream`. Request records may include `method`, `url` or `path_and_query`, `headers`, and `body`. Response and event records may include `status`, `headers`, `body`, `payload`, or `data`. WebSocket records may include `direction` and `message`. SSE records may include `stream`, `text`, `body`, or `data`.

Example JSONL request record:

```json
{"type":"request","method":"POST","url":"https://chatgpt.com/backend-api/codex/responses","headers":{"Authorization":"Bearer live-token","User-Agent":"codex-cli/local"},"body":{"stream":true,"previous_response_id":"resp_live"}}
```

The compat capture scrubber redacts auth headers, API keys, cookies, token-like fields, account IDs, session IDs, response IDs, request IDs, call IDs, UUIDs, and timestamp-like fields. Use `--stdout` to inspect output before writing, `--output` to choose an exact fixture path, `--check` to compare against an existing fixture, and `--self-test` to run the built-in scrub regression check.

`npm run test:fast` prebuilds cargo test binaries with `cargo test --no-run` before starting parallel cargo test shards when `CI` is not set. This local warmup reduces misleading cargo build lock waits from many child processes trying to compile the same test binaries at once. CI defaults are preserved: when `CI=true`, prebuild is off unless explicitly enabled with `npm run test:fast -- --prebuild`.

Use `npm run test:fast -- --no-prebuild` when measuring cold parallel behavior or debugging cargo scheduling itself. Seeing Cargo print `Blocking waiting for file lock` during local parallel shards usually means another cargo process is compiling or writing the shared target/cache directory, not that a test has deadlocked.

## Domain Boundary Guard

Run `npm run ci:domain-boundary-guard` after changing `crates/prodex-domain/Cargo.toml` or moving security/accounting identifiers between crates. The guard keeps `prodex-domain` pure by rejecting HTTP, CLI, database, async-runtime, transport, filesystem/process/network, and provider/runtime dependencies and source imports; use `node scripts/ci/domain-boundary-guard.mjs --self-test` after changing the guard itself.

Use `npm run ci:gateway-security-smoke` after touching gateway admin authentication, OIDC/JWKS admin auth, data-plane auth separation, or gateway usage/budget enforcement. It runs the focused `gateway_admin_auth` and `gateway_usage` `prodex-app` suites that cover missing/unknown role fallback, stale JWKS request-path behavior, admin-token rejection on inference routes, and gateway usage/accounting auth invariants.
