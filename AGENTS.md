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

## Runtime Proxy Rules

The runtime proxy is the most sensitive part of the project.

### Required affinity behavior

These bindings must remain reliable:

- `previous_response_id -> profile`
- `x-codex-turn-state -> profile`

If a request continues an existing chain, it should stay on the owning profile whenever possible.

### Rotation boundaries

Safe auto-rotate is allowed only before a request/stream is committed:

- before the first successful unary response is accepted
- before the first streaming response is committed
- before a quota-blocked or overload response is returned to Codex

Do not rotate mid-stream after model output has started.

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
- Fresh pre-commit selection may use a short-lived per-profile in-flight load signal to avoid creating hotspots.
- Fresh pre-commit selection may also enforce a short per-profile in-flight cap so new work fails fast instead of piling more pressure onto a busy account.
- Temporary connect/read/stream transport failures may place a profile into short transport backoff.
- Temporary overload or repeated transport flakiness may add a short-lived profile health penalty that affects only new candidate selection.
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

### Unary compact path

Remote compaction uses the unary endpoint:

- `/responses/compact`

This path should remain eligible for safe retry/rotate on temporary overload or quota exhaustion, while other unary errors should pass through unchanged.

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

## Observability

Runtime proxy diagnostics are written to `/tmp`.

Useful files:

- `/tmp/prodex-runtime-latest.path`: pointer to the latest runtime log
- `/tmp/prodex-runtime-*.log`: per-run proxy logs

If a user reports a stall, inspect the latest runtime log before changing behavior blindly.
Look for:

- `runtime_proxy_queue_overloaded`
- `runtime_proxy_active_limit_reached`
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
