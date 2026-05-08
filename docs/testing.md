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
npm run test:fast
npm run test:serial
npm run ci:preflight
npm run ci:release-hygiene
npm run ci:release-metadata-guard
npm run ci:runtime-hotpath-guard
npm run ci:crate-boundary
npm run ci:churn-hygiene
npm run release:prepare
npm run changelog:check
npm run docs:lint
npm run ci:runtime-manifest
npm run test:runtime-smoke
node scripts/compat/check-upstream-baseline.mjs
node scripts/compat/watch-upstream.mjs
npm run compat:capture -- --input /path/to/capture.jsonl --name codex_live_sample
node scripts/ci/runtime-proxy-shard.mjs
npm run ci:runtime-stress -- --suite stress
npm run ci:runtime-stress -- --suite serialized
npm run ci:runtime-stress -- --suite continuation
node scripts/ci/runtime-env-parallel.mjs --runs 2 --test-threads 4
cargo test -q --workspace --all-features -- --test-threads=1
```

Use `npm run test:fast -- --jobs 4` for local safe lanes that can run as independent child processes. Use `npm run test:serial -- --suite all` for global-env, runtime, continuation, and quarantine lanes that must stay serialized with `--test-threads=1`.

Use `npm run test:runtime-smoke` for a small local runtime invariant suite before broad runtime work. It runs curated log JSON/marker, header preservation, selection affinity, stale continuation, websocket local pressure, and tuning snapshot checks from the shared runtime manifest without changing the broad runtime or stress suites.

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

Use `npm run ci:preflight` before pushing broad runtime or release-adjacent changes. It runs release hygiene, size, crate-boundary, runtime hot-path, churn hygiene, version/docs/runtime-manifest/fmt/cargo-check guards, clippy with warnings denied, and the fast test lane. Churn hygiene fails on actionable violations by default; use `npm run ci:preflight -- --churn-report-only` or `PRODEX_PREFLIGHT_CHURN_REPORT_ONLY=1 npm run ci:preflight` only for exploratory local runs. Add `-- --serial` when a change needs the serialized runtime/global-state lane too, or `-- --dry-run` to inspect the command plan. For local historical audits only, `-- --churn-range old..HEAD --churn-ignore-before latest-tag` keeps churn enforcement focused on commits after the newest version tag inside the selected range.

Use `npm run ci:release-hygiene` for the full release gate. By default it checks `HEAD`; use `-- --range main..HEAD` or `-- --base origin/main --head HEAD` for branch ranges. The runner executes metadata-only, version metadata, empty release commit, duplicate release version, tag/changelog, and historical fixture checks as one serial gate.

Use `npm run ci:release-metadata-guard` for the focused metadata-only check. By default it checks `HEAD`; use `-- --range main..HEAD` for a branch range, or `-- --staged --assume-release` before committing a release bump. The guard fails only when a release-like commit changes both version metadata files such as `Cargo.toml`, `Cargo.lock`, npm package manifests, `README.md`, or `QUICKSTART.md`, and non-metadata files.

Use `npm run ci:runtime-hotpath-guard` after proxy hot-path work. It strips `#[cfg(test)]` Rust items, scans runtime proxy hot-path targets for blocking `std::fs`/`fs::` disk operations, file opens, `spawn_blocking`, and OS `thread::spawn`, then permits only narrow allowlisted existing cases with rationale.

Use `npm run ci:crate-boundary` after adding workspace dependencies. It parses Cargo manifests and fails on direct dependency edges from focused/helper crates into app orchestration, terminal rendering, or runtime-proxy-incompatible layers.

Use `PRODEX_RUNTIME_PROXY_BENCH_CHECK=1 PRODEX_RUNTIME_PROXY_BENCH_THRESHOLD_FILE=scripts/ci/runtime-proxy-bench-thresholds.json cargo bench --locked --features bench-support --bench runtime_proxy_hot_paths` after benchmark-sensitive proxy changes. Checked-in runtime proxy bench scales use the policy encoded in `scripts/ci/runtime-proxy-bench-thresholds.json`: calibrate against the max observed median, keep a 25% nanosecond margin, require at least 15 scale points above observed required scale, avoid per-case scales below 50%, and suppress suggestions smaller than 5 scale points. `node scripts/ci/benchmark-calibration.mjs <bench-log>` applies the same defaults so local or pasted CI logs do not silently tighten tiny benchmark cases to machine-specific values.

Use `npm run ci:churn-hygiene` for a lightweight churn gate. It fails on actionable violations by default and checks `HEAD~1..HEAD` when available. Use `-- --report-only` or `PRODEX_CHURN_HYGIENE_REPORT_ONLY=1 npm run ci:churn-hygiene` only for exploratory local reports, or pass `-- --range main..HEAD`, `-- --staged`, custom `--max-files`, `--max-lines`, `--max-behavior-files`, or `--max-file-lines` values for stricter local review. Historical release/tag ranges may include already-reviewed broad churn; for those local audits, use `-- --range old..HEAD --ignore-before latest-tag` to report the original range while checking only changes after the newest version tag inside the selected range. A pinned reviewed baseline is still accepted when a specific non-release review point is intentional. Do not use baseline options for PR or push guard runs, where the full new range should stay enforced. Release metadata-only sweeps across Cargo manifests, npm package manifests, versioned install snippets, or changelog files may exceed the file-count threshold, but line-count, largest-file, behavior-file, and subject checks still apply.

Use `npm run release:prepare` before release work. It checks version/doc sync, available lockfile consistency, generated changelog freshness, docs lint, upstream compatibility baseline, runtime manifest, cargo fmt, and full Rust test binary compilation without publishing.
The default test-compile guard runs `cargo test --locked --workspace --all-targets --all-features --no-run` so workspace lib, bin, integration test, example, and benchmark targets stay compile-covered. Use `npm run release:prepare -- --no-cargo-test` to skip test binary compilation and run `cargo check --locked --workspace --all-targets --all-features` instead.

Use `npm run ci:runtime-manifest` after adding or renaming runtime proxy tests. New `main_internal_tests::runtime_proxy_` tests should normally get a targeted `RUNTIME_CI_TEST_CASES` entry; only add or rely on `RUNTIME_CI_BROAD_SHARD_FILTERS` when a broad CI shard intentionally owns that whole module or prefix. Broad shard filters must mirror the `label|filter` entries in the `main-internal-runtime-proxy` matrix in `.github/workflows/ci.yml` and must not match tests outside runtime CI ownership.

When changing `prodex-context` audit, prose compression, or command-output context-saver helpers, run `cargo test -q -p prodex-context`. If the `prodex context compact-output` CLI surface changes, also run a focused CLI parse/handler test. The command-output helpers are pure and opt-in; they should not require runtime proxy tests unless a separate runtime integration changes.

When changing `prodex info` runtime tuning output, run the focused `cargo test -q runtime_tuning_snapshot_reports_effective_policy_and_env_values -- --test-threads=1` check so env, policy, and default-derived values stay aligned.

Use `node scripts/compat/check-upstream-baseline.mjs` before changing runtime proxy assumptions. It is offline and verifies that `scripts/compat/upstream-baseline.json` still records the critical upstream Codex files, Responses/compact routes, SSE/websocket stream events, and headers that Prodex preserves or replaces. Baseline format version 2 also records semantic check groups that tie route, header, event, and co-occurrence expectations back to specific upstream files instead of relying only on flat `required_contains` tokens.

Use `node scripts/compat/watch-upstream.mjs` when network access is available. It fetches current upstream Codex critical files from the latest release tag, falls back to `main` only when the tag raw file is unavailable, and reports missing required tokens or semantic groups as upstream drift.

Use `npm run compat:capture -- --input capture.jsonl --name codex_live_sample` to convert offline captured Codex or Claude traffic into scrubbed replay fixtures under `crates/prodex-app/tests/fixtures/compat_replay`. The tool does not capture traffic and never uses the network; it only normalizes local JSON, JSONL, or text input into a deterministic fixture.

Compat capture input should usually be JSONL, one record per line. Supported record `type` values are `request`, `response`, `event`, `websocket_message`, and `sse_stream`. Request records may include `method`, `url` or `path_and_query`, `headers`, and `body`. Response and event records may include `status`, `headers`, `body`, `payload`, or `data`. WebSocket records may include `direction` and `message`. SSE records may include `stream`, `text`, `body`, or `data`.

Example JSONL request record:

```json
{"type":"request","method":"POST","url":"https://chatgpt.com/backend-api/codex/responses","headers":{"Authorization":"Bearer live-token","User-Agent":"codex-cli/local"},"body":{"stream":true,"previous_response_id":"resp_live"}}
```

The compat capture scrubber redacts auth headers, API keys, cookies, token-like fields, account IDs, session IDs, response IDs, request IDs, call IDs, UUIDs, and timestamp-like fields. Use `--stdout` to inspect output before writing, `--output` to choose an exact fixture path, `--check` to compare against an existing fixture, and `--self-test` to run the built-in scrub regression check.

`npm run test:fast` prebuilds cargo test binaries with `cargo test --no-run` before starting parallel cargo test shards when `CI` is not set. This local warmup reduces misleading cargo build lock waits from many child processes trying to compile the same test binaries at once. CI defaults are preserved: when `CI=true`, prebuild is off unless explicitly enabled with `npm run test:fast -- --prebuild`.

Use `npm run test:fast -- --no-prebuild` when measuring cold parallel behavior or debugging cargo scheduling itself. Seeing Cargo print `Blocking waiting for file lock` during local parallel shards usually means another cargo process is compiling or writing the shared target/cache directory, not that a test has deadlocked.
