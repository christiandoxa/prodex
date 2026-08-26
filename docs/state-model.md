# Prodex State Model

This document defines the local state surfaces that must stay predictable across
CLI commands, runtime proxy workers, diagnostics, and future setup/repair flows.

## Authoritative State

`$PRODEX_HOME/state.json` is the durable profile registry.

It owns:

- `active_profile`
- profile metadata and managed `CODEX_HOME` paths
- `response_profile_bindings`
- `session_profile_bindings`
- runtime quota snapshots and short-lived profile health/backoff data

Runtime state updates must stay merge-safe. Writers must preserve unrelated
fields from other processes and avoid replacing the whole file from stale
in-memory snapshots.

## Shared Codex State

Prodex profile auth stays isolated under `$PRODEX_HOME/profiles/<name>`.
Codex-owned shared files default to native `~/.codex` unless
`PRODEX_SHARED_CODEX_HOME` points elsewhere.

Shared Codex state includes:

- `history.jsonl`
- `sessions/`
- `config.toml`
- `environments.toml`
- plugins, skills, prompts, and memory files

These files are Codex-owned compatibility surfaces. Prodex may link, mirror, or
prepare them for a launch, but should not reinterpret chat history or plugin
state as profile authority.

## Runtime Bindings

The runtime proxy must preserve hard affinity:

- `previous_response_id -> profile`
- `x-codex-turn-state -> profile`
- `session_id -> profile` for session-scoped unary routes

These bindings are authoritative for continuations. Fresh selection heuristics,
transport backoff, profile health, and in-flight caps must not override them.

## Quota And Health

Quota snapshots and profile health are advisory for fresh selection unless a
request has not yet committed to a profile. Quota classification should remain
specific: generic upstream `429` is not account quota unless the payload clearly
identifies quota or rate-limit exhaustion.

Route-scoped health should remain route-scoped. A `responses` transport penalty
must not automatically poison `/responses/compact` or websocket selection unless
there is explicit evidence.

## Runtime Logs

Runtime logs are diagnostics, not source of truth. Default path is the OS temp
directory, overridable by `PRODEX_RUNTIME_LOG_DIR` or `runtime.log_dir` in
`policy.toml`.

Important files:

- `prodex-runtime-latest.path`
- `prodex-runtime-*.log`
- runtime broker registry and lease files

`prodex doctor --runtime` summarizes recent logs. When adding runtime markers,
update the runtime doctor marker registry and tests so diagnostics remain typed.

## Setup And Optional-Tool State

`prodex setup` is a local reconciler for install surfaces. It may ensure Prodex
directories exist and validate externally installed optional tools. It must not
download tools or mutate profile auth, shared Codex history, runtime bindings,
or user-managed tool installations.

Optional-tool activation is limited to the temporary launch overlay. Version,
source, commit, and digest metadata in each managed installation must match the
release-vetted metadata compiled into Prodex.

## Expose Instance State

An expose process is intentionally process-local. It owns one captured initial
workspace, one ephemeral capability digest, one MCP server identity, one
Quick Tunnel child, one resolved Super configuration, one bounded run manager,
and one set of child process groups. It does not persist the capability, run
table, task text, event history, or active model/effort configuration.

Multiple expose processes may share the authoritative `PRODEX_HOME` profile,
quota, health, cooldown, auto-rotation, and durable model-preference state
where the existing logical scope requires it. Merge-safe writers remain the
only owners of that shared state. A later preference update must not mutate a
running expose instance; per-run MCP overrides apply only to that run.

## Invariants

- Profile auth isolation is stronger than convenience.
- Shared Codex history remains upstream-compatible.
- Continuation affinity beats selection heuristics.
- Runtime log diagnostics must not become required for request success.
- Setup/repair commands must be idempotent and safe under dirty user state.
- Dry-run output must describe writes before performing them.
