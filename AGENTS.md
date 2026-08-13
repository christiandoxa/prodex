# AGENTS.md

This file applies to the entire repository. Keep agent guidance actionable and
repository-specific; the maintained documents linked below own detailed architecture,
policy, threat-model, and test contracts.

## Scope and source of truth

When rules conflict, use this order:

1. Current repository behavior and safety invariants.
2. Enforced CI guards, test runners, and `package.json` scripts.
3. Maintained Prodex documentation: [architecture](docs/architecture.md),
   [testing](docs/testing.md), [runtime policy](docs/runtime-policy.md), and
   [threat model](docs/threat-model.md).
4. This file.
5. Transferable lessons from upstream Codex guidance.
6. Generic Rust preferences.

`Cargo.toml` and each crate manifest define workspace membership and dependency ownership.
Do not infer a boundary from a directory name alone. Repository prose, comments, examples,
fixtures, and new documentation must be in English.

## Project summary and ownership

Prodex is a Rust workspace whose CLI wraps Codex and manages isolated `CODEX_HOME` profiles.
It also builds dedicated gateway and control-plane entrypoints. Keep composition roots thin:

- `src/main.rs`, `src/bin/`, and `src/lib.rs` wire entrypoints and compatibility facades.
- `prodex-cli` owns Clap parsing, help, defaults, and command normalization.
- `prodex-app` owns CLI command routing, application composition, profile flows, runtime
  launch, and live orchestration. It must not become the home for reusable domain, storage,
  rendering, provider, or transport-neutral logic.
- `prodex-application` owns side-effect-free use-case plans and ports. Concrete transports,
  providers, and storage adapters belong in composition roots.
- `prodex-domain` owns pure identifiers, security context, policy, accounting, and governance
  decisions. It must not depend on HTTP, CLI, database drivers, filesystem/process APIs,
  providers, or network clients.
- `prodex-runtime-proxy` owns proxy boundary types, classifiers, affinity/health/admission
  helpers, compatibility transforms, and bounded transport executors. Keep it hot-path-safe;
  live launch and app orchestration remain in `prodex-app`.
- `prodex-runtime-*` crates own focused runtime policy, state, quota, launch, diagnostics,
  broker, and provider-runtime boundaries; inspect the owning crate before editing.
- `prodex-gateway-core` owns HTTP-neutral admission/routing contracts; `prodex-gateway-http`
  owns framework-neutral HTTP policy; `prodex-gateway-server` owns bounded async Hyper/TLS
  serving. Do not move policy into the server adapter.
- `prodex-storage` owns adapter-neutral commands and contracts. Driver-free SQL/plans belong
  in `prodex-storage-postgres`, `prodex-storage-redis`, or `prodex-storage-sqlite`; async or
  blocking execution belongs in the matching `*-runtime` adapter. DDL and migrations stay out
  of request-serving paths.
- `prodex-terminal-ui` stays generic: terminal dimensions, layout, text, and rendering helpers;
  app- or runtime-specific report models stay in report crates or `prodex-app`.

Focused crates must not depend upward on `prodex-app`. Reuse an existing inward crate before
adding a new one. Introduce a crate only for a durable ownership boundary with more than one
real consumer and a useful dependency-direction benefit; otherwise add a focused private
module in the owning crate.

## Before changing code

- Find existing helpers, types, tests, and the owning boundary before writing new code.
- Trace callers and the end-to-end flow before changing shared logic; fix the shared root cause.
- State material assumptions, then make the smallest behavior-preserving change. Avoid
  speculative abstractions, configurability, dependencies, and drive-by refactors.
- Keep new modules private by default and re-export only deliberate public contracts.
- When extracting code, move its tests, invariants, and module/type documentation with it.

## Runtime Proxy Rules

The runtime proxy is the most sensitive part of the project. Preserve the detailed contract in
`docs/architecture.md` and `docs/runtime-policy.md`.

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
- Pass through upstream status, body, headers, and streaming payload after a response exists.
- If Prodex fails before any upstream response, return local `503 service_unavailable` rather than synthetic quota `429`.

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
- Keep request and stream hot paths non-blocking: do not add disk I/O, broad reads, mutex-held I/O, or unbounded blocking/thread work; use async transport and bounded background work.
- Do not let transport backoff override hard affinity for an in-flight continuation that already owns a profile.
- Do not let temporary profile health penalties override hard affinity for an in-flight continuation that already owns a profile.
- Do not let temporary in-flight load heuristics override hard affinity for an in-flight continuation that already owns a profile.
- Do not let the per-profile in-flight hard cap override hard affinity for an in-flight continuation that already owns a profile.
- Keep pre-commit candidate selection bounded in both time and attempts so the proxy fails fast when the whole pool is unhealthy.
- Runtime state saves must not block request/stream commit paths.
- Do not print runtime notices while the Codex TUI runs; send them to the resolved runtime log directory instead.
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

## Rust readability and API design

- Prefer Clippy-supported inline `format!` arguments, collapsed conditionals, and method
  references when they improve clarity; do not churn equivalent code without a benefit.
- Prefer exhaustive `match` at domain, protocol, security, persistence, and compatibility
  boundaries. Do not hide new cases behind broad wildcard arms.
- Avoid opaque positional booleans, numbers, and ambiguous `Option` values. Prefer enums,
  newtypes, named methods, builders, or typed option structs that make call sites explicit.
- Newly introduced public traits require documentation of their role, contract, and expected
  implementation behavior. Prefer native Rust trait futures with an explicit `Send` contract
  when the toolchain and API shape support it; do not add `async_trait` merely for convenience.
- Use typed `Result` errors and explicit trust-boundary validation. Do not add unjustified
  `unwrap()`, `expect()`, `panic!()`, `todo!()`, or `unimplemented!()` to production paths.
- Do not add a one-call-site helper unless it encodes a real invariant. Put async tracing on
  the owning function/method where practical, after checking delegated instrumentation.
- Keep public crate APIs minimal. Avoid test-only public APIs and production helpers created
  solely to make a test compile. Use the standard library and existing dependencies first.

## Module and change-size discipline

`size-guard.mjs`, `size-guard-allowlist.json`, `churn-hygiene.mjs`, and their CI wiring are
the source of truth. Current default size limits are 850 lines for production Rust, 860 for
tests/benches, and 770 as the production cohesion threshold; the near-limit budget is 32
files and a directory may have at most 9 near-limit production siblings.

- Prefer new production modules under roughly 700 lines, excluding tests.
- Treat files at or above the 770-line cohesion threshold as extraction candidates. Do not add
  substantial behavior to an oversized allowlisted file without first evaluating extraction.
- Allowlist caps are narrow, reviewed ratchets, not permanent architecture approval. Lower or
  remove stale caps after splits; never weaken the guard merely to pass CI.
- Churn hygiene defaults are 35 changed files, 25 behavior files, 1200 changed lines, and 500
  changed lines in the largest file. Use the guard’s actual range and defaults; do not import
  upstream thresholds. Divide large non-mechanical work into coherent behavior-preserving
  stages. Large structural extraction must remain clearly mechanical and satisfy the guard’s
  declaration rules.

## Security, secrets, and supply chain

- Treat HTTP, CLI, configuration, provider, storage, and credential input as untrusted. Validate
  at the boundary, use typed plans/IDs, and preserve authorization and tenant isolation.
- Keep secret values out of domain models, logs, errors, fixtures, environment-derived examples,
  persisted diagnostics, and provider debug output. Production gateway credentials use typed
  `SecretRef`/projected resolution; secret or JWKS network fetches must not occur on request
  paths. Do not put prompts, request bodies, cookies, bearer values, credentials, tenant
  secrets, or filesystem paths in route traces or metric labels.
- PostgreSQL is durable enterprise accounting state; Redis is for rate limiting, short-lived
  cache, and rebuildable coordination, not the billing ledger. Keep migrations/DDL explicit and
  outside request handling.
- Tracked examples and tests must use reserved domains (`example.com`), generic paths such as
  `/home/test-user`, synthetic profile names, and fake IDs/tokens. Never copy real emails,
  paths, overlay/session/attachment IDs, logs, auth data, credentials, or customer identifiers.
- Never pipe a remote script into a shell. Pin external CI actions and downloaded tools to full
  immutable SHAs or explicit versions. Review dependency and lockfile changes; add no dependency
  when an existing crate or the standard library is sufficient.
- Do not commit or push unless explicitly authorized.

## Testing strategy

- Every behavior change needs meaningful observable regression coverage. Prefer integration or
  boundary tests when behavior crosses crates, processes, protocols, persistence, CLI surfaces,
  gateway contracts, or runtime proxy transport.
- Prefer whole-object equality when it states the contract clearly. Do not add tests that only
  restate static constants or deleted behavior. Keep substantial new test modules in focused
  sibling files; do not grow an oversized test file for convenience.
- Avoid process-global environment mutation. Prefer injection or explicit context. When it is
  unavoidable, use the existing `TestEnvVarGuard`/`EnvGuard` plus the shared environment lock;
  never add ad hoc unguarded `set_var`/`remove_var`.
- Use deterministic local mocks and the existing terminal renderer/width-aware assertions for
  output. Do not add a snapshot dependency merely to copy upstream practice.
- Runtime tests must isolate temp homes, `CODEX_HOME`, `CLAUDE_CONFIG_DIR`, runtime log paths,
  ports, broker state, and continuation state. Keep global-env/runtime/continuation cases in
  serialized process shards with `--test-threads=1`; prefer independent processes for parallel
  coverage. Keep runtime test manifests in sync with `npm run ci:runtime-manifest`.
- Live provider, credential-bearing, network, or cost-bearing tests require explicit
  authorization. Use deterministic fixtures or the offline compatibility gate by default.

## Compatibility and platform review

Before changing a shared or public contract, search for breakage across:

- CLI flags, defaults, help, exit status, and terminal output;
- `policy.toml`, environment variables, serialized state, and persisted affinity/bindings;
- gateway/control-plane HTTP status, headers, bodies, routes, pagination, and auth behavior;
- provider translations, upstream Responses/compact/SSE/WebSocket semantics, and preserved
  metadata;
- public Rust crate APIs and the npm gateway SDK;
- log markers, metric names, diagnostics, release/install behavior, and versioned docs.

For upstream compatibility changes, run the local offline baseline and replay checks. Use the
network watch only when current upstream drift is relevant; never use live provider traffic as a
compatibility fixture. Supported behavior must remain correct on Linux, macOS, and Windows
unless explicitly platform-specific. Account for path and filesystem semantics, permissions
and secure file handling, process identity/termination, terminal behavior, line endings, shell
assumptions, and executable resolution. Use the native CI lanes when local validation cannot
cover another OS.

## Validation command matrix

Use the narrowest meaningful checks first, then broaden for shared or high-risk changes. Report
exact commands and results; do not claim a skipped or failed command passed.

- Markdown or repository-guidance changes: `npm run docs:lint`, `npm run test:changed`, and
  `git diff --check`.
- Rust module/API changes: `cargo fmt --check`, a focused `cargo test --locked -q -p <crate>`
  command, and `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`
  when shared/API or CI-sensitive code is touched.
- Runtime proxy changes: `npm run test:runtime-smoke`,
  `cargo test --locked -q -p prodex-app --lib 'main_internal_tests::runtime_proxy_' -- --test-threads=1`,
  `npm run ci:runtime-hotpath-guard`, `npm run ci:runtime-manifest`, and
  `npm run compat:offline-gate`; add bounded load smoke for admission/latency changes.
- Dependency or boundary changes: `npm run ci:crate-boundary` plus the owning domain,
  gateway, storage, auth, provider, or application boundary guard.
- Broad, release-adjacent, or high-risk changes: `npm run ci:preflight`; add
  `npm run test:serial -- --suite all` when global state/runtime paths changed and
  `npm run test:full -- --timings` when full workspace evidence is required.
- Release metadata: after changing `Cargo.toml`, run `npm run npm:sync-version`, review
  `Cargo.lock`, and use `npm run release:prepare` or the dry-run release flow before publishing.

## Documentation and release behavior

Keep detailed contracts in the canonical docs above; do not turn this file into a full
architecture manual. Update README/QUICKSTART, CLI help, compatibility fixtures, or the owning
docs when user-visible behavior changes. Quota is a Prodex-owned screen: human views refresh
every 5 seconds by default, `--raw` and `--once` are one-shot modes, the prior snapshot remains
visible during refresh, and `--all` preserves sort order while reporting hidden rows; snapshot
style tests should use `--once`.

For an authorized release, follow the repository release scripts and
`.github/workflows/standalone-release.yml`: synchronize version metadata, lockfiles, tests, and
docs as required by their guards. The default release path is GitHub/container artifacts, not
npm or crates.io publication. Do not publish, tag, commit, or push without explicit authorization.

## Agent handoff

Report the outcome first, then changed files, adopted compatibility/safety decisions, exact
validation commands with pass/fail/skip results, redacted failure evidence and remaining risk.
State material assumptions or ambiguity. Confirm whether a commit or push occurred; without
explicit authorization, it must not have occurred.
