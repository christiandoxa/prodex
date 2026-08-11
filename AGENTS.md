# AGENTS.md

This file applies to the entire repository.

## Project Summary

`prodex` is a Rust workspace whose primary CLI wraps `codex` and manages multiple isolated
`CODEX_HOME` profiles. The root package also builds dedicated `prodex-gateway` and
`prodex-control-plane` entrypoints.

The Cargo workspace is split across focused crates and modules. `Cargo.toml` is the source of
truth for membership and package boundaries. Keep this summary limited to paths agents commonly
need:

- `src/main.rs`: thin binary composition root
- `src/lib.rs`: root facade and dedicated-server helpers
- `src/bin/`: dedicated gateway and control-plane composition roots
- `crates/prodex-app/`: application orchestration, command routing, profile flows, and runtime glue
- `crates/prodex-runtime-*/`: runtime proxy, state, policy, quota, launch, and diagnostics code
- `crates/prodex-gateway-*/`: gateway contracts, HTTP adaptation, and server composition
- `crates/prodex-storage-*/`: storage contracts, backend configuration, migrations, and adapters
- other `crates/prodex-*/`: focused reusable boundaries; inspect the owning crate before editing
- `README.md` and `QUICKSTART.md`: user-facing documentation

## Core Principles

When changing `prodex`, keep these invariants intact:

1. The runtime proxy should be as transport-transparent as possible.
   - Let `codex` own reconnect, WebSocket fallback, and stream UX.
   - Do not invent new stream semantics unless strictly necessary.

2. Auto-rotate must remain built in to the proxy.
   - Profile/account selection is a `prodex` responsibility.
   - Transport behavior should remain as close as possible to upstream Codex.
   - Reliability improvements must not weaken affinity or allow mid-stream rotation.

3. Do not redefine upstream ChatGPT errors unless the proxy itself failed before any upstream response existed.
   - Prefer pass-through for upstream HTTP status, body, and stream payloads.

4. Do not print anything to the terminal while the Codex TUI is running.
   - Preflight output before launch is fine.
   - Runtime notices must go to log files, not stdout/stderr.

5. Repository prose must stay in English.

6. Runtime hot paths must stay non-blocking as much as possible.
   - Do not reintroduce disk I/O, broad file reads, or unbounded thread spawning into the request/stream hot path.
   - Prefer async transport and bounded background work over ad hoc blocking behavior.

7. Prodex-owned screens should be terminal-responsive.
   - Prefer adapting to the current terminal width instead of assuming a fixed 110-character layout.
   - Live views may also adapt to terminal height when that improves readability without hiding critical state silently.
   - If a live view refreshes in place, keep the previous snapshot visible until the next snapshot is ready to render.

8. Tracked content must not contain real personal or operational identifiers.
   - Use reserved domains such as `example.com`, generic paths such as `/home/test-user`, generic profile names, and synthetic UUIDs or tokens in docs, tests, fixtures, and examples.
   - Never copy real email-derived profile names, home or `CODEX_HOME` paths, overlay IDs, attachment IDs, session IDs, logs, auth data, or credentials into the repository.
   - Preserve intentional public project ownership metadata. Git commit authors may use the maintainer identity configured locally, including a real name and verified email address; continue to scan the tracked diff for secrets and PII before release.

## Before Coding

- State assumptions and material ambiguities before implementation; ask only when no safe default exists.
- Find existing helpers, types, and patterns before adding new ones.
- Trace all callers and the end-to-end flow before changing shared logic; fix the root cause at the shared boundary.
- Put new logic in its owning crate or module. Keep `src/main.rs` and `src/bin/` as thin composition roots.
- For upstream compatibility changes, inspect the relevant upstream implementation and compatibility fixtures/tests before changing behavior.
- Prefer the minimum behavior-preserving change. Avoid speculative abstractions, configurability, dependencies, and drive-by refactors.
- Do not assume existing code or patterns are correct; prioritize correctness, security, performance, readability, and maintainability in that order.

## Code and Comments

- Write comments only for non-obvious invariants, security or transport behavior, `unsafe` code, tool directives, deliberate `ponytail:` trade-offs, or TODO/FIXME items with a concrete reason and follow-up reference.
- Keep comments concise and do not remove unrelated existing comments during a focused change.
- Prefer Rust idioms: typed errors with `Result`, exhaustive `match`, and explicit handling at trust boundaries. Avoid unjustified `unwrap()` or `expect()` in production paths.
- Use the standard library and existing dependencies before adding a new dependency; record the reason when a new dependency is necessary.

## Workflow and Supply-Chain Safety

- Never pipe a remote script into a shell. Download artifacts to a file, verify checksums, then install.
- Pin external CI actions and tools to immutable full SHAs or explicit versions; never use `latest` or `stable` for downloaded artifacts.
- Never print, copy, commit, or publish secrets, auth data, customer names, private incident identifiers, or real operational identifiers.
- Never commit or push changes unless explicitly requested.

## Runtime Proxy Rules

The runtime proxy is the most sensitive part of the project.

### Required affinity behavior

These bindings must remain reliable:

- `previous_response_id -> profile`
- `x-codex-turn-state -> profile`
- `session_id -> profile` for session-scoped unary routes such as remote compact

If a request continues an existing chain, it should stay on the owning profile whenever possible.

### Rotation boundaries

Safe auto-rotate is allowed only before a request/stream is committed:

- before the first successful unary response is accepted
- before the first streaming response is committed
- before a quota-blocked or overload response is returned to Codex

Do not rotate mid-stream after model output has started.

For fresh requests without hard affinity, a single last-chance attempt on the current profile is acceptable when only local selection heuristics were exhausted.
That fallback must not override:

- `previous_response_id` ownership
- `x-codex-turn-state` ownership
- `session_id` ownership for an existing session-scoped route
- mid-stream no-rotate rules

### Transport transparency

Keep proxy behavior close to upstream Codex:

- WebSocket upstream sessions should be reused where appropriate.
- HTTP/SSE should stream as directly as possible.
- If upstream transport breaks, prefer letting Codex observe a natural transport failure.

### Reliability guardrails

The runtime proxy should remain conservative and durable under poor networks and many terminals:

- Keep long-lived request handling bounded; avoid unbounded `thread::spawn` patterns in acceptor paths.
- Treat transport failures separately from quota failures.
- Treat short-lived profile health as a separate signal from quota backoff and transport backoff.
- Treat short-lived profile health as endpoint-specific where possible, so `responses`, `/responses/compact`, and websocket transport can degrade independently for fresh selection.
- Fresh pre-commit selection may use a short-lived per-profile in-flight load signal to avoid creating hotspots.
- Fresh pre-commit selection may also enforce a short per-profile in-flight cap so new work fails fast instead of piling more pressure onto a busy account.
- Local proxy admission may also enforce short lane-aware caps so `responses`, `compact`, `websocket`, and other unary traffic do not starve each other.
- Lane-aware admission limits are for fresh local admission only; they must not override hard affinity for an existing continuation that already owns a profile.
- Lane-aware admission should prefer protecting the main `responses` lane from starvation by bursty `compact`, websocket, or other unary traffic.
- Temporary connect/read/stream transport failures may place a profile into short transport backoff.
- Temporary overload or repeated transport flakiness may add a short-lived profile health penalty that affects only new candidate selection.
- Endpoint-specific health penalties must not globally poison unrelated fresh routes unless there is a deliberate reason to do so.
- Do not treat a generic upstream `429 Too Many Requests` body as account-specific quota unless the upstream payload explicitly identifies a quota/rate-limit error code such as `insufficient_quota` or `rate_limit_exceeded`.
- If pre-commit selection fails before any upstream response exists, prefer a local `503 service_unavailable` over a synthetic `429 insufficient_quota`.
- Do not let transport backoff override hard affinity for an in-flight continuation that already owns a profile.
- Do not let temporary profile health penalties override hard affinity for an in-flight continuation that already owns a profile.
- Do not let temporary in-flight load heuristics override hard affinity for an in-flight continuation that already owns a profile.
- Do not let the per-profile in-flight hard cap override hard affinity for an in-flight continuation that already owns a profile.
- Keep pre-commit candidate selection bounded in both time and attempts so the proxy fails fast when the whole pool is unhealthy.
- Runtime state saves must not block request/stream commit paths.
- Cross-process state persistence should remain merge-safe for:
  - `active_profile`
  - `last_run_selected_at`
  - `response_profile_bindings`
  - `session_profile_bindings`

### Unary compact path

Remote compaction uses the unary endpoint:

- `/responses/compact`

This path should remain eligible for safe retry/rotate on temporary overload or quota exhaustion, while other unary errors should pass through unchanged.
When `session_id` is present and already bound to a profile, compact should prefer that owning profile before fresh unary selection.

For `429` on unary paths:

- only rotate when the upstream payload clearly signals quota exhaustion
- plain-text or generic `429` responses should pass through unchanged

## Headers and Metadata

Preserve upstream request metadata unless it is truly hop-by-hop or auth that must be replaced for the selected profile.

Important headers to preserve when present:

- `session_id`
- `x-openai-subagent`
- `x-codex-turn-state`
- `x-codex-turn-metadata`
- `x-codex-beta-features`
- request `User-Agent`

Headers that are intentionally replaced by the proxy for the selected profile:

- `Authorization`
- `ChatGPT-Account-Id`

Headers that may be skipped as transport-local:

- `Host`
- `Connection`
- `Content-Length`
- `Transfer-Encoding`
- `Upgrade`
- `sec-websocket-*`

## Quota UX

`prodex quota` is a Prodex-owned screen, not a Codex TUI path.

- By default, `prodex quota` should refresh continuously every 5 seconds.
- This default applies to both single-profile quota views and `prodex quota --all`.
- `prodex quota --raw` should remain one-shot.
- `prodex quota --once` is the explicit one-shot escape hatch for human-facing quota views.
- During a live quota refresh, the previous snapshot should stay visible until the next snapshot is fully ready to render.
- The live `prodex quota --all` view may truncate to the current terminal height, but it must preserve the existing sort order, show the top rows that fit, and surface how many profiles are hidden.
- When changing quota behavior, keep integration tests and docs aligned so snapshot-style tests use `--once`.

## Observability

Runtime proxy diagnostics are written to the resolved runtime log directory.
The default is the OS temp directory, which is usually `/tmp` on Linux, but `PRODEX_RUNTIME_LOG_DIR` or `runtime.log_dir` in `policy.toml` can override it.

Useful files:

- `<runtime-log-dir>/prodex-runtime-latest.path`: pointer to the latest runtime log
- `<runtime-log-dir>/prodex-runtime-*.log`: per-run proxy logs

If a user reports a stall, inspect the latest runtime log before changing behavior blindly.
Use `prodex doctor --runtime --json` when the effective directory is not known; its `log_path` field points at the sampled log.
Look for:

- `runtime_proxy_queue_overloaded`
- `runtime_proxy_active_limit_reached`
- `runtime_proxy_lane_limit_reached`
- `runtime_proxy_overload_backoff`
- `profile_inflight_saturated`
- `upstream_connect_*`
- `first_upstream_chunk`
- `first_local_chunk`
- `stream_read_error`
- `profile_retry_backoff`
- `profile_transport_backoff`
- `profile_inflight`
- `profile_health`
- `selection_plan`
- `precommit_budget_exhausted`
- `state_save_*`

If `selection_plan` appears without a later `selection_pick` or `selection_keep_current`, inspect its `ready=`, `fallback=`, `cold_start_jobs=`, and `sync_probe_mode=` fields before changing upstream-facing behavior.
If `profile_health` appears, inspect its `route=` value before changing selection behavior globally.
If `runtime_proxy_lane_limit_reached` appears, inspect its `lane=` value before changing upstream-facing behavior.
Repeated `lane=responses` markers suggest the main model lane is saturated locally; repeated non-`responses` markers suggest a side lane is consuming proxy capacity.
If `runtime_proxy_active_limit_reached` or `profile_inflight_saturated` appears repeatedly without matching transport or quota markers, suspect local concurrency pressure before changing upstream-facing behavior.

## Tests and Verification

- Every feature or bug fix needs a meaningful test that checks observable behavior, not coverage alone.
- A regression test should fail before the fix and pass only when the broken behavior is corrected.
- Extend the nearest existing test module by default; create a new test file only when no suitable mapped test exists.
- Use deterministic local mocks or fixtures for network-facing proof. Live provider calls require explicit approval because they can expose credentials and incur cost.
- Report the exact verification commands and relevant redacted output for user-visible or transport-facing changes.
- Use the narrowest checks that cover the change. Use the full suite, Clippy gate, and release build for cross-crate, release, or CI-sensitive changes.

## Key Commands

Format:

```bash
cargo fmt
```

Check formatting without modifying files:

```bash
cargo fmt --check
```

Run cheap checks selected from changed paths:

```bash
npm run test:changed
```

Run documentation checks after changing Markdown or user-facing docs:

```bash
npm run docs:lint
```

Run the CI Clippy gate:

```bash
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
```

Run the focused runtime proxy tests:

```bash
cargo test -q -p prodex-app --lib 'main_internal_tests::runtime_proxy_' -- --test-threads=1
```

Run the full test suite:

```bash
npm run test:full -- --timings
```

Summarize the latest runtime log:

```bash
prodex doctor --runtime
```

Show quota as a one-shot snapshot:

```bash
prodex quota --all --once
```

Build the local binary after runtime changes:

```bash
cargo build --release --locked
```

If you changed dependencies or release metadata, refresh and review the lockfile before publishing.
Prefer the release scripts for version metadata, use targeted updates where possible, and use a
broad `cargo update` only when intentionally refreshing the dependency graph:

```bash
cargo update
cargo update --manifest-path fuzz/Cargo.toml
```

## Editing Guidance

- Prefer narrow, behavior-preserving changes in the owning crate or module.
- Update `README.md`, `QUICKSTART.md`, CLI help, or other relevant docs in the same change when user-visible behavior changes.
- Add regression tests for every runtime proxy bug fix.
- When touching runtime persistence, add or update tests for multi-process-safe merge behavior.
- When touching transport recovery, add or update tests for both quota backoff and transport backoff behavior.
- When touching runtime candidate selection, add or update tests for:
  - hard affinity preservation
  - transport backoff handling
  - temporary profile health handling
  - bounded pre-commit retry/selection behavior
- When touching proxy logic, compare behavior against upstream Codex in:
  - `codex-rs/core/src/client.rs`
  - `codex-rs/core/src/compact_remote.rs`
  - `codex-rs/codex-api/src/sse/responses.rs`
  - `codex-rs/codex-api/src/endpoint/responses_websocket.rs`

## Release Notes

This project has been released frequently.

Release versioning on the `0.x` line follows the current release plan. Do not encode historical
version examples in this file as release rules.

After bumping `Cargo.toml`, sync workspace version metadata with:

```bash
npm run npm:sync-version
```

If asked to publish:

1. bump `Cargo.toml`
2. run `npm run npm:sync-version`
3. update `Cargo.lock`
4. run tests
5. publish the standalone binaries through `.github/workflows/standalone-release.yml`

Do not publish to npm or crates.io in the default release path.
The workspace currently requires publishing many internal `prodex-*` crates before the root `prodex` crate, which can hit crates.io new-crate rate limits and create partial releases.
Keep release publishing GitHub-only unless another registry is explicitly re-enabled with a deliberate plan.

The `.github/workflows/standalone-release.yml` workflow creates or refreshes the matching standalone GitHub Release for the plain `0.x.y` tag and must not publish npm packages. The release title should stay version-only, matching the tag, rather than `prodex v<version>`. It should also keep versioned documentation metadata synced when the release commit matches `origin/main`.

If asked to commit, use a conventional commit message.
