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

3. Do not redefine upstream ChatGPT errors unless the proxy itself failed before any upstream response existed.
   - Prefer pass-through for upstream HTTP status, body, and stream payloads.

4. Do not print anything to the terminal while the Codex TUI is running.
   - Preflight output before launch is fine.
   - Runtime notices must go to log files, not stdout/stderr.

5. Repository prose must stay in English.

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

Reinstall the local binary after runtime changes:

```bash
cargo install --path . --force
```

## Editing Guidance

- Prefer narrow, behavior-preserving changes in `src/main.rs`.
- Add regression tests for every runtime proxy bug fix.
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
