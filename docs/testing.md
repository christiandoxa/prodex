# Testing

Prodex test speed should come from process-level sharding first, not from making every Rust test process run with high harness concurrency.

## Strategy

- Fast lanes should split safe suites across independent CI jobs or processes.
- Serial lanes should contain tests that mutate process-global state, depend on runtime lifecycle, or share local runtime resources.
- Runtime proxy tests should prefer manifest-owned shards so fragile cases are easy to quarantine without hiding them from CI.
- The runtime manifest uses explicit category tags for `runtime:parallel-safe`, `runtime:serial`, `runtime:stress`, `runtime:env`, and `runtime:quarantine`.
- Full serial coverage should remain available as a scheduled or manual safety net.

Independent process shards are preferred because each process can own its environment variables, temp homes, runtime log directory, artifacts, and background tasks. Inside risky runtime or global-env shards, keep Rust harness scheduling serial with `--test-threads=1`.

## Parallel Safety

Treat a test as parallel-safe only when it avoids process-global state, fixed shared paths, fixed ports, shared profile homes, shared broker state, and long-lived background work that can outlive the test.

Treat a test as serial or quarantined when it touches any of these:

- `std::env` mutation, including model/provider overrides
- current working directory changes
- default runtime log paths, including `/tmp/prodex-runtime-latest.path`
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
npm run docs:lint
npm run ci:runtime-manifest
node scripts/compat/check-upstream-baseline.mjs
node scripts/ci/runtime-proxy-shard.mjs
npm run ci:runtime-stress -- --suite stress
npm run ci:runtime-stress -- --suite serialized
npm run ci:runtime-stress -- --suite continuation
node scripts/ci/runtime-env-parallel.mjs --runs 2 --test-threads 4
cargo test -q --all-features -- --test-threads=1
```

Use `npm run test:fast -- --jobs 4` for local safe lanes that can run as independent child processes. Use `npm run test:serial -- --suite all` for global-env, runtime, continuation, and quarantine lanes that must stay serialized with `--test-threads=1`.

Use `node scripts/compat/check-upstream-baseline.mjs` before changing runtime proxy assumptions. It is offline and verifies that `scripts/compat/upstream-baseline.json` still records the critical upstream Codex files, Responses/compact routes, SSE/websocket stream events, and headers that Prodex preserves or replaces.

`npm run test:fast` prebuilds cargo test binaries with `cargo test --no-run` before starting parallel cargo test shards when `CI` is not set. This local warmup reduces misleading cargo build lock waits from many child processes trying to compile the same test binaries at once. CI defaults are preserved: when `CI=true`, prebuild is off unless explicitly enabled with `npm run test:fast -- --prebuild`.

Use `npm run test:fast -- --no-prebuild` when measuring cold parallel behavior or debugging cargo scheduling itself. Seeing Cargo print `Blocking waiting for file lock` during local parallel shards usually means another cargo process is compiling or writing the shared target/cache directory, not that a test has deadlocked.
