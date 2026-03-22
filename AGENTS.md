# AGENTS.md

This file applies to the entire repository.

## Project Summary

`prodex` is a single-binary Rust CLI that wraps `codex` and manages multiple isolated `CODEX_HOME` profiles.

The project currently lives mostly in one file:

- `src/main.rs`: CLI, profile management, quota logic, runtime proxy, tests
- `README.md`: full user-facing documentation
- `QUICKSTART.md`: shorter installation and usage guide

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

Runtime proxy diagnostics are written to `/tmp`.

Useful files:

- `/tmp/prodex-runtime-latest.path`: pointer to the latest runtime log
- `/tmp/prodex-runtime-*.log`: per-run proxy logs

If a user reports a stall, inspect the latest runtime log before changing behavior blindly.
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
- `precommit_budget_exhausted`
- `state_save_*`

If `profile_health` appears, inspect its `route=` value before changing selection behavior globally.
If `runtime_proxy_lane_limit_reached` appears, inspect its `lane=` value before changing upstream-facing behavior.
Repeated `lane=responses` markers suggest the main model lane is saturated locally; repeated non-`responses` markers suggest a side lane is consuming proxy capacity.
If `runtime_proxy_active_limit_reached` or `profile_inflight_saturated` appears repeatedly without matching transport or quota markers, suspect local concurrency pressure before changing upstream-facing behavior.

## Key Commands

Format:

```bash
cargo fmt
```

Run the focused runtime proxy tests:

```bash
cargo test -q runtime_proxy_ -- --test-threads=1
```

Run the full test suite:

```bash
cargo test -q -- --test-threads=1
```

Summarize the latest runtime log:

```bash
prodex doctor --runtime
```

Show quota as a one-shot snapshot:

```bash
prodex quota --all --once
```

Reinstall the local binary after runtime changes:

```bash
cargo install --path . --force
```

If you changed dependencies or release metadata, refresh the lockfile before publishing:

```bash
cargo update
```

## Editing Guidance

- Prefer narrow, behavior-preserving changes in `src/main.rs`.
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

If asked to publish:

1. bump `Cargo.toml`
2. update `Cargo.lock`
3. run tests
4. run `cargo publish --dry-run`
5. run `cargo publish`

If asked to commit, use a conventional commit message.
