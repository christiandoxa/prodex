# Prodex Architecture Map

This repository is a Rust workspace with focused crates under `crates/` plus the root `prodex`
package. Its three thin composition roots are `src/main.rs`, `src/bin/prodex-gateway.rs`, and
`src/bin/prodex-control-plane.rs`; `src/lib.rs` is their shared compatibility facade.

## Command Path

Normal command flow:

```text
argv
  -> prodex-cli
  -> prodex-app command_dispatch
  -> prodex-app command handler
  -> focused helper crates
  -> prodex-app reports / prodex-terminal-ui for human output
```

Dedicated enterprise flows enter through `prodex-gateway` or `prodex-control-plane`, then cross
the gateway/control-plane adapter crates into `prodex-application` and backend-neutral
`prodex-storage` contracts. Pure policy and identity decisions remain in `prodex-domain`.

Dependency direction is inward:

- domain types do not depend on HTTP, processes, providers, or concrete storage;
- application use cases depend on ports, not concrete adapters;
- provider and storage adapters accept validated plans and point toward those ports;
- `prodex-app` and the dedicated binaries compose concrete adapters but must not
  duplicate authentication, authorization, admission, accounting, or routing policy;
- shared DTOs move to an existing inward crate only when multiple real consumers need them;
- no focused crate may depend back on `prodex-app`.

Key crates and files:

- `prodex-cli`: clap argument model, help text, and default `prodex <args>` to `prodex run <args>` rewrite.
- `prodex-app`: orchestration layer. `command_dispatch.rs` routes parsed commands to handler modules.
- `prodex-app::reports`: report models/rendering used only by app-owned screens.
- `prodex-terminal-ui`: terminal width/layout helpers. Keep it generic; it should not know app or runtime proxy internals.

Common command edit points:

- `prodex session`: `crates/prodex-app/src/app_commands/session.rs`, `crates/prodex-session-store`, `crates/prodex-app/src/reports/session.rs`.
- `prodex quota`: `crates/prodex-app/src/app_commands/quota.rs`, `crates/prodex-app/src/quota_support`, `prodex-quota`, `prodex-runtime-quota`, `crates/prodex-app/src/reports`.
- `prodex doctor`: `crates/prodex-app/src/app_commands/doctor.rs`, `crates/prodex-app/src/runtime_doctor`, `prodex-runtime-doctor`, `prodex-runtime-broker-log`.
- Profile commands: `crates/prodex-app/src/profile_commands`, `prodex-profile-identity`, `prodex-profile-export`, `prodex-shared-codex-fs`.
- Release metadata: root `Cargo.toml`, crate manifests, lockfile, npm manifests, and versioned docs snippets. Parent release flow wires those together; avoid touching release metadata unless assigned.

## ChatGPT Expose Composition

The ChatGPT convenience path is an additive composition around the existing
browser expose subsystem:

```text
ChatGPT
  -> Cloudflare Quick Tunnel, user-managed Cloudflare hostname, or OpenAI Secure MCP Tunnel
  -> loopback HTTP listener
  -> exact capability + public Host policy
  -> Streamable HTTP JSON MCP adapter
  -> process-local run manager
  -> normal `prodex s exec -` child
  -> shared Super/runtime/profile routing
```

`prodex s expose` captures one canonical initial workspace and configures Super
before generating a capability. Quick Tunnel mode binds `127.0.0.1:0` and starts
one directly invoked, user-config-isolated `cloudflared tunnel --protocol auto
--url ...` child. Existing Tunnel mode binds the user-selected loopback port,
uses the exact validated hostname, and does not start or stop cloudflared. The
public default admits only `/pdx/v1/<capability>/mcp`; `prodex expose --tunnel`
and `prodex s expose --tunnel` retain the explicit browser-terminal behavior.
`prodex s expose --tunnel-provider openai` supervises the official tunnel client
for MCP connectivity only: its browser terminal stays local and no public
browser URL is generated.

The MCP layer owns ingress validation, protocol schemas, bounded run lifecycle,
and redacted status/events/results. It does not own model selection, provider
routing, quota, auto-rotation, Codex continuation, optional tools, or account
selection. A run task is delivered over stdin, not argv, and the child uses the
same Super argument/runtime path as local execution.

An expose process is the isolation unit for active state: capability digest,
workspace, server identity, tunnel, run manager, event/output rings, and child
process groups are not shared. Multiple processes may still share the existing
merge-safe `PRODEX_HOME` profile/quota/health and durable-preference state. The
MCP endpoint is intentionally not a filesystem sandbox beyond the underlying
Super permission model; documentation must reflect that full-access caveat.

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
- An owner-bound WebSocket continuation may release affinity after a pre-commit usage limit only by asking Codex to replay full context; its delta request is never sent to another profile.
- Rotate only before commit: before first accepted unary response, before first committed stream response, or before returning quota/overload to Codex.
- Do not rotate mid-stream after model output starts.
- Pass through upstream status/body/stream payloads unless the proxy failed before any upstream response existed.
- Keep request and stream commit paths non-blocking. Avoid disk I/O, broad reads, unbounded thread spawn, and terminal output while Codex TUI runs.
- Keep endpoint health scoped where possible: `responses`, `/responses/compact`, websocket, and other lanes should not poison each other without a deliberate reason.
- Reload runtime policy only from publication consumers or other bounded background paths. Validate the candidate before atomically replacing the normalized per-root cache entry; failed reloads leave the previous entry untouched.

Runtime proxy edit points:

- Live orchestration: `crates/prodex-app/src/runtime_proxy`.
- Side-effect-free proxy helpers, the bounded route-decision trace, and gateway request-constraint planning: `crates/prodex-runtime-proxy`.
- Provider catalog limits, request-requirement parsing, token estimation, and pure model constraint evaluation: `crates/prodex-provider-core`.
- Gateway live-plan orchestration and the authenticated, side-effect-free route-explain HTTP/dashboard edge: `crates/prodex-app/src/runtime_launch/proxy_startup`.
- Launch planning: `crates/prodex-app/src/runtime_launch`, `prodex-runtime-launch`, `prodex-runtime-claude`, `prodex-runtime-anthropic`.
- Policy and tuning: `prodex-runtime-policy`, `prodex-runtime-tuning`,
  `crates/prodex-app/src/runtime_policy.rs`, and the [runtime policy reference](runtime-policy.md).
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
  read-only projected external-secret provider described by
  [ADR 0009](enterprise-governance/adrs/0009-external-secret-vault.md). Explicit
  production gateway policy resolves typed credential references through that
  provider at startup and rejects raw CLI/environment sources as recorded in
  the [security test matrix](security-test-matrix.md).
- `prodex-profile-export`: encrypted import/export envelopes.

Persistence rules:

- Cross-process saves must remain merge-safe for active profile, last-run timestamps, response bindings, and session bindings.
- Runtime state saves must not block request/stream commit paths.
- Add merge/persistence regression tests when changing state shape or save behavior.
- PostgreSQL recovery must pass the Docker-backed logical dump/restore gate for
  recovery-point age, restore time, tenant-table completeness, accounting
  consistency, post-backup exclusion, and non-owner RLS isolation; see the
  [storage, HA, backup, and DR contract](enterprise-governance/09-storage-ha-backup-and-dr.md).

## Quota, Doctor, Observability

Quota and diagnostics path:

```text
prodex-app command handler
  -> prodex-quota / prodex-runtime-quota / prodex-runtime-doctor
  -> prodex-app reports / prodex-terminal-ui
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
- `prodex log` is the canonical short form of `prodex log stream`; both use one live handler.
  `prodex log upstream` remains the explicit upstream-payload mode. All three share the human TUI
  title `Prodex Log`. Their right-aligned t/s field is output tokens per active generation second,
  sourced from existing token-usage timing; prompt/cache tokens, payload bytes, TTFT, and unrelated
  processes are not included. After a valid measurement the latest numeric rate remains visible
  while idle; before any measurement the field is `— t/s`.

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
- `prodex-optional-tools`: side-effect-free optional-tool discovery and validation plus temporary-overlay activation.
- `prodex-housekeeping`: cleanup and duplicate-detection helpers.
- `prodex-context`: context audit and compression helpers. Rust owns filesystem/process
  collection, ANSI/line normalization, Unicode trim compatibility, and diagnostic classifiers;
  the Mojo-enabled build owns bounded UTF-8 duplicate grouping and critical-signal row planning
  through `prodex-mojo-core`.

The rich Mojo core is an additive ABI v6 boundary for deterministic domain work. The active
context, provider fallback, route-alias policy, provider-routing, and Smart Context planning
paths pass bounded non-secret UTF-8 views and caller-owned record/output arenas to Mojo. Mojo
constructs typed diagnostic, identifier, policy, route-candidate, and context-item values,
performs normalization/parsing/grouping/ranking, and returns validated structured records. Rust
retains the external Serde/TOML/JSON boundaries, tenant/security checks, credentials, IO,
transport, persistence, and public presentation. A Mojo-enabled result error is hard failure;
the feature-off Rust implementation is a separate target/oracle and never a runtime fallback.

## Boundary Guard

Run:

```bash
node scripts/ci/crate-boundary-guard.mjs
```

The guard parses workspace Cargo manifests and fails on direct dependency edges that obviously point upward or across boundaries:

- focused crates depending on `prodex-app`
- low-level helper crates depending on app, report, terminal, runtime launch, or runtime proxy layers
- `prodex-terminal-ui` depending on app or runtime proxy layers
- `prodex-runtime-proxy` depending on app or terminal/report/orchestration crates

When a rule fires, prefer one of these fixes:

- Move shared DTOs or pure helpers down into a focused helper crate.
- Keep app-specific report rendering in `prodex-app::reports`; keep generic terminal layout in `prodex-terminal-ui`.
- Call orchestration upward from `prodex-app`, not from helper crates.
- Keep hot-path runtime proxy helpers side-effect-free in `prodex-runtime-proxy`.
