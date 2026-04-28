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
cargo test -q --all-features -- --test-threads=1
```

Use `npm run test:fast -- --jobs 4` for local safe lanes that can run as independent child processes. Use `npm run test:serial -- --suite all` for global-env, runtime, continuation, and quarantine lanes that must stay serialized with `--test-threads=1`.

Use `npm run test:runtime-smoke` for a small local runtime invariant suite before broad runtime work. It runs curated log JSON/marker, header preservation, selection affinity, stale continuation, websocket local pressure, and tuning snapshot checks from the shared runtime manifest without changing the broad runtime or stress suites.

Use `npm run ci:preflight` before pushing broad runtime or release-adjacent changes. It runs release metadata/docs/runtime-manifest/fmt/cargo-check guards, clippy with warnings denied, and the fast test lane. Add `-- --serial` when a change needs the serialized runtime/global-state lane too, or `-- --dry-run` to inspect the command plan.

Use `npm run release:prepare` before release work. It checks version/doc sync, available lockfile consistency, generated changelog freshness, docs lint, upstream compatibility baseline, runtime manifest, cargo fmt, and full Rust test binary compilation without publishing.
The default test-compile guard runs `cargo test --locked --all-targets --all-features --no-run` so lib, bin, integration test, example, and benchmark targets stay compile-covered. Use `npm run release:prepare -- --no-cargo-test` to skip test binary compilation and run `cargo check --locked --all-targets --all-features` instead.

Use `npm run ci:runtime-manifest` after adding or renaming runtime proxy tests. New `main_internal_tests::runtime_proxy_` tests should normally get a targeted `RUNTIME_CI_TEST_CASES` entry; only add or rely on `RUNTIME_CI_BROAD_SHARD_FILTERS` when a broad CI shard intentionally owns that whole module or prefix. Broad shard filters must mirror the `label|filter` entries in the `main-internal-runtime-proxy` matrix in `.github/workflows/ci.yml` and must not match tests outside runtime CI ownership.

When changing `prodex info` runtime tuning output, run the focused `cargo test -q runtime_tuning_snapshot_reports_effective_policy_and_env_values -- --test-threads=1` check so env, policy, and default-derived values stay aligned.

Use `node scripts/compat/check-upstream-baseline.mjs` before changing runtime proxy assumptions. It is offline and verifies that `scripts/compat/upstream-baseline.json` still records the critical upstream Codex files, Responses/compact routes, SSE/websocket stream events, and headers that Prodex preserves or replaces. Baseline format version 2 also records semantic check groups that tie route, header, event, and co-occurrence expectations back to specific upstream files instead of relying only on flat `required_contains` tokens.

Use `node scripts/compat/watch-upstream.mjs` when network access is available. It fetches current upstream Codex critical files from the latest release tag, falls back to `main` only when the tag raw file is unavailable, and reports missing required tokens or semantic groups as upstream drift.

Use `npm run compat:capture -- --input capture.jsonl --name codex_live_sample` to convert offline captured Codex or Claude traffic into scrubbed replay fixtures under `tests/fixtures/compat_replay`. The tool does not capture traffic and never uses the network; it only normalizes local JSON, JSONL, or text input into a deterministic fixture.

Compat capture input should usually be JSONL, one record per line. Supported record `type` values are `request`, `response`, `event`, `websocket_message`, and `sse_stream`. Request records may include `method`, `url` or `path_and_query`, `headers`, and `body`. Response and event records may include `status`, `headers`, `body`, `payload`, or `data`. WebSocket records may include `direction` and `message`. SSE records may include `stream`, `text`, `body`, or `data`.

Example JSONL request record:

```json
{"type":"request","method":"POST","url":"https://chatgpt.com/backend-api/codex/responses","headers":{"Authorization":"Bearer live-token","User-Agent":"codex-cli/local"},"body":{"stream":true,"previous_response_id":"resp_live"}}
```

The compat capture scrubber redacts auth headers, API keys, cookies, token-like fields, account IDs, session IDs, response IDs, request IDs, call IDs, UUIDs, and timestamp-like fields. Use `--stdout` to inspect output before writing, `--output` to choose an exact fixture path, `--check` to compare against an existing fixture, and `--self-test` to run the built-in scrub regression check.

`npm run test:fast` prebuilds cargo test binaries with `cargo test --no-run` before starting parallel cargo test shards when `CI` is not set. This local warmup reduces misleading cargo build lock waits from many child processes trying to compile the same test binaries at once. CI defaults are preserved: when `CI=true`, prebuild is off unless explicitly enabled with `npm run test:fast -- --prebuild`.

Use `npm run test:fast -- --no-prebuild` when measuring cold parallel behavior or debugging cargo scheduling itself. Seeing Cargo print `Blocking waiting for file lock` during local parallel shards usually means another cargo process is compiling or writing the shared target/cache directory, not that a test has deadlocked.
