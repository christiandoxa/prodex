# Prodex Architecture Map

This repository is a Rust workspace with 39 focused crates under `crates/` plus the root `prodex` package. The root binary is intentionally thin: `src/main.rs` calls `prodex::main_entry()`, and `src/lib.rs` re-exports `prodex_app` as a compatibility shim.

## Command Path

Normal command flow:

```text
argv
  -> prodex-cli
  -> prodex-app command_dispatch
  -> prodex-app command handler
  -> focused helper crates
  -> prodex-app-reports / prodex-terminal-ui for human output
```

Key crates and files:

- `prodex-cli`: clap argument model, help text, and default `prodex <args>` to `prodex run <args>` rewrite.
- `prodex-app`: orchestration layer. `command_dispatch.rs` routes parsed commands to handler modules.
- `prodex-app-reports`: reusable report models/rendering for app-owned screens.
- `prodex-terminal-ui`: terminal width/layout helpers. Keep it generic; it should not know app or runtime proxy internals.

Common command edit points:

- `prodex session`: `crates/prodex-app/src/app_commands/session.rs`, `crates/prodex-session-store`, `prodex-app-reports/src/session.rs`.
- `prodex quota`: `crates/prodex-app/src/app_commands/quota.rs`, `crates/prodex-app/src/quota_support`, `prodex-quota`, `prodex-runtime-quota`, `prodex-app-reports`.
- `prodex doctor`: `crates/prodex-app/src/app_commands/doctor.rs`, `crates/prodex-app/src/runtime_doctor`, `prodex-runtime-doctor`, `prodex-runtime-broker-log`.
- Profile commands: `crates/prodex-app/src/profile_commands`, `prodex-profile-identity`, `prodex-profile-export`, `prodex-shared-codex-fs`.
- Release metadata: root `Cargo.toml`, crate manifests, lockfile, npm manifests, and versioned docs snippets. Parent release flow wires those together; avoid touching release metadata unless assigned.

## Runtime Proxy Hot Path

Runtime launch and proxy flow:

```text
prodex run / prodex caveman / prodex claude
  -> prodex-app runtime_launch
  -> prodex-runtime-launch plans child process and env
  -> prodex-app runtime_proxy owns live transport orchestration
  -> prodex-runtime-proxy supplies side-effect-free classifiers, boundary types, and helpers
  -> upstream Codex / ChatGPT / Claude-compatible runtime
```

Hot path invariants:

- Preserve hard affinity: `previous_response_id -> profile`, `x-codex-turn-state -> profile`, and session-scoped `session_id -> profile`.
- Rotate only before commit: before first accepted unary response, before first committed stream response, or before returning quota/overload to Codex.
- Do not rotate mid-stream after model output starts.
- Pass through upstream status/body/stream payloads unless the proxy failed before any upstream response existed.
- Keep request and stream commit paths non-blocking. Avoid disk I/O, broad reads, unbounded thread spawn, and terminal output while Codex TUI runs.
- Keep endpoint health scoped where possible: `responses`, `/responses/compact`, websocket, and other lanes should not poison each other without a deliberate reason.
- Reload runtime policy only from publication consumers or other bounded background paths. Validate the candidate before atomically replacing the normalized per-root cache entry; failed reloads leave the previous entry untouched.

Runtime proxy edit points:

- Live orchestration: `crates/prodex-app/src/runtime_proxy`.
- Side-effect-free proxy helpers and tests: `crates/prodex-runtime-proxy`.
- Launch planning: `crates/prodex-app/src/runtime_launch`, `prodex-runtime-launch`, `prodex-runtime-claude`, `prodex-runtime-anthropic`.
- Policy and tuning: `prodex-runtime-policy`, `prodex-runtime-tuning`, `crates/prodex-app/src/runtime_policy.rs`, and ADR 1055.
- Benchmark support: `prodex-bench-support`, root `benches/`.

## State And Persistence

State flow:

```text
prodex-app runtime/profile/session handlers
  -> prodex-state / prodex-runtime-state data models
  -> prodex-runtime-store / prodex-session-store merge and compaction helpers
  -> prodex-app runtime_persistence for process integration
```

Key crates:

- `prodex-state`: profile and app state models.
- `prodex-runtime-state`: runtime lane counters, bindings, snapshots, and scheduled-save models.
- `prodex-runtime-store`: merge and compaction helpers for persisted runtime state.
- `prodex-session-store`: persisted shared Codex session metadata helpers.
- `prodex-secret-store`: development storage primitives and the bounded,
  read-only projected external-secret provider described by ADR 1058.
- `prodex-profile-export`: encrypted import/export envelopes.

Persistence rules:

- Cross-process saves must remain merge-safe for active profile, last-run timestamps, response bindings, and session bindings.
- Runtime state saves must not block request/stream commit paths.
- Add merge/persistence regression tests when changing state shape or save behavior.
- PostgreSQL recovery must pass the Docker-backed dump/restore gate for RPO,
  RTO, tenant-table completeness, accounting consistency, point-in-time
  exclusion, and non-owner RLS isolation; see ADR 1057.

## Quota, Doctor, Observability

Quota and diagnostics path:

```text
prodex-app command handler
  -> prodex-quota / prodex-runtime-quota / prodex-runtime-doctor
  -> prodex-app-reports / prodex-terminal-ui
  -> terminal for Prodex-owned screens only
```

Key crates:

- `prodex-quota`: quota API models, auth helpers, and quota rendering helpers.
- `prodex-runtime-quota`: runtime quota snapshots, summaries, adapter helpers, and sort keys.
- `prodex-runtime-doctor`: runtime diagnostics parsing, summaries, suggestions, and rendering.
- `prodex-runtime-log`: runtime log path and marker helpers.
- `prodex-runtime-broker`, `prodex-runtime-broker-log`, `prodex-runtime-metrics`: broker registry DTOs, log parsing, and Prometheus rendering.
- `prodex-audit-log`: append/query/render helpers for structured audit events.
- `prodex-redaction`: shared diagnostic redaction helpers.

Observability rules:

- Prodex-owned screens may print before launching Codex or in standalone commands.
- Runtime notices while Codex TUI runs go to log files only.
- If runtime stalls, inspect latest runtime log markers before changing selection or transport behavior.

## Session, Profile, And Shared Codex FS

Profile and filesystem flow:

```text
profile/session command
  -> prodex-app profile/session handler
  -> prodex-core path discovery
  -> prodex-shared-codex-fs for shared Codex file operations
  -> prodex-state / prodex-session-store for persisted metadata
```

Key crates:

- `prodex-core`: path discovery and common filesystem helpers.
- `prodex-shared-codex-fs`: shared Codex home file operations.
- `prodex-profile-identity`: account identity parsing and profile-name normalization.
- `prodex-codex-config`: Codex config parsing helpers.
- `prodex-caveman-assets`: embedded Codex/Claude Caveman plugin assets and home/config preparation.
- `prodex-housekeeping`: cleanup and duplicate-detection helpers.
- `prodex-context`: context audit and compression helpers.

## Boundary Guard

Run:

```bash
node scripts/ci/crate-boundary-guard.mjs
```

The guard parses workspace Cargo manifests and fails on direct dependency edges that obviously point upward or across boundaries:

- focused crates depending on `prodex-app`
- low-level helper crates depending on app, report, terminal, runtime launch, or runtime proxy layers
- report/render crates depending on app
- `prodex-terminal-ui` depending on app or runtime proxy layers
- `prodex-runtime-proxy` depending on app or terminal/report/orchestration crates

When a rule fires, prefer one of these fixes:

- Move shared DTOs or pure helpers down into a focused helper crate.
- Keep terminal/report rendering in `prodex-app-reports` or `prodex-terminal-ui`.
- Call orchestration upward from `prodex-app`, not from helper crates.
- Keep hot-path runtime proxy helpers side-effect-free in `prodex-runtime-proxy`.
