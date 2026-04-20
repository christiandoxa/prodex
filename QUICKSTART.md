# Quick Start

One OpenAI profile pool. Codex, Claude Code, Caveman mode, and optional Claude-Mem overlays.

Use `prodex` for Codex CLI, `prodex caveman` for Caveman-mode Codex, `prodex claude` for Claude Code, and add the `mem` prefix when you want to reuse an existing Claude-Mem install with either front end. All of these commands run on top of the same OpenAI-backed Prodex profile pool.

## Requirements

- An OpenAI account, plus at least one logged-in Prodex profile
- Codex CLI if you want to use `prodex`
- Claude Code (`claude`) if you want to use `prodex claude`
- Optional: `claude-mem` if you want `prodex caveman mem`, `prodex claude mem`, or `prodex claude caveman mem`

If you install `@christiandoxa/prodex` from npm, the pinned Codex runtime dependency `@openai/codex@0.121.0` is installed for you. Claude Code is still a separate CLI and should already be installed when you use `prodex claude`.

If you want the `mem` path, install Claude-Mem separately with the upstream installer:

```bash
npx claude-mem install --ide codex-cli
npx claude-mem install --ide claude-code
npx claude-mem start
```

## Install

Install from npm:

```bash
npm install -g @christiandoxa/prodex
```

Or install from [crates.io](https://crates.io/crates/prodex):

```bash
cargo install prodex
```

## Update

Check your installed version first:

```bash
prodex --version
```

The current local version in this repo is `0.25.0`:

```bash
npm install -g @christiandoxa/prodex@0.25.0
cargo install prodex --force --version 0.25.0
```

Dependency status in this repo:

- The npm runtime dependency is pinned to `@openai/codex@0.121.0` in the workspace package manifest
- Run `cargo update` whenever dependency metadata changes so the published lockfile stays in sync
- Versioned install snippets in this guide and `README.md` are synced from `Cargo.toml`

If you want to switch from a Cargo-installed binary to npm:

```bash
cargo uninstall prodex
npm install -g @christiandoxa/prodex
```

## 1. Create your first profile

If your shared Codex home already contains a login:

```bash
prodex profile import-current main
```

If you already use GitHub Copilot CLI on this machine and want Prodex to track that provider identity:

```bash
prodex profile import copilot
prodex profile import copilot --name copilot-main --activate
```

Or create a profile through the normal Codex login flow:

```bash
prodex login
prodex login --device-auth
```

If you want a fixed profile name first:

```bash
prodex profile add second
prodex login --profile second
```

## 2. Inspect the pool

```bash
prodex profile list
prodex profile export
prodex profile import copilot
prodex quota --all
prodex info
```

`prodex quota --all` refreshes live by default. Use `--once` when you want a single snapshot:

```bash
prodex quota --all --once
```

Backup or move profiles:

```bash
prodex profile export
prodex profile export backup.json
prodex profile import backup.json
prodex profile remove --all
```

`prodex profile export` exports all configured profiles by default and asks whether the bundle should use optional password protection.

`prodex profile import copilot` records the logged-in Copilot account and provider endpoint in Prodex while leaving the token in Copilot's own keychain/config storage. The imported profile appears in the pool and export/import flow, but `prodex run`, `prodex login`, `prodex logout`, and `prodex quota` still target OpenAI/Codex profiles only.

## 3. Run Codex CLI with `prodex`

`prodex` without a subcommand is shorthand for `prodex run`.

```bash
prodex
prodex run
prodex run --profile second
prodex run 019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9
prodex exec "review this repo"
printf 'context from stdin' | prodex run exec "summarize this"
```

Use this path when you want Codex CLI itself to be the front end. Prodex keeps transport behavior close to direct Codex while handling profile selection, quota preflight, continuation affinity, and safe pre-commit rotation.

## 4. Run Codex with `prodex caveman`

```bash
prodex caveman
prodex caveman mem
prodex caveman --profile second
prodex caveman exec "review this repo in caveman mode"
```

Use this path when you want Codex itself as the front end but want Caveman mode preloaded from the upstream Caveman plugin. Prodex launches Caveman from a temporary overlay `CODEX_HOME` so the base profile home stays unchanged after the session exits.

Use `prodex caveman mem` when you also want an existing Claude-Mem Codex install to follow the selected Prodex session path instead of watching only the default `~/.codex/sessions` tree.

## 5. Run Claude Code with `prodex claude`

```bash
prodex claude -- -p "summarize this repo"
prodex claude mem -- -p "recall past work on this repo"
prodex claude caveman
prodex claude caveman mem
prodex claude caveman -- -p "summarize this repo briefly"
prodex claude --profile second -- -p --output-format json "show the latest diff"
```

Use this path when you want Claude Code to be the front end while Prodex still routes requests through your OpenAI-backed profile pool.

Use `prodex claude caveman` when you want the same Claude path but with the upstream Caveman plugin loaded through Claude's session-local `--plugin-dir` support. Prodex keeps the plugin bundle stable under `.prodex`, and the adapted Caveman hooks read and write the Prodex-managed `CLAUDE_CONFIG_DIR` instead of your global `~/.claude`.

Use `prodex claude mem` when you want to reuse an existing upstream Claude-Mem Claude Code install, and use `prodex claude caveman mem` when you want Caveman mode and Claude-Mem together in the same Claude session.

What changes on this path:

- Claude Code talks to a local Anthropic-compatible Prodex proxy
- managed profiles link `CLAUDE_CONFIG_DIR` into shared Prodex Claude state
- the first managed Claude launch imports your existing `~/.claude` and `~/.claude.json` when present
- the initial Claude model follows the shared Codex `config.toml` model when available
- Claude's `opus`, `sonnet`, and `haiku` entries are pinned to representative GPT models
- Prodex seeds Claude's picker with the Prodex GPT catalog via Claude-native placeholder model IDs
- Claude `max` effort maps to OpenAI `xhigh` when the selected GPT model supports it
- supported picker entries use Claude-native placeholders so Claude can expose native effort controls

Useful overrides:

```bash
PRODEX_CLAUDE_BIN=/path/to/claude prodex claude -- -p "hello"
PRODEX_CLAUDE_MODEL=gpt-5.4 prodex claude -- -p "hello"
PRODEX_CLAUDE_MODEL=gpt-5.2 PRODEX_CLAUDE_REASONING_EFFORT=xhigh prodex claude -- -p "hello"
```

## 6. Switch profiles explicitly

```bash
prodex use --profile main
prodex current
```

## 7. Debug the runtime

```bash
prodex cleanup
prodex doctor
prodex audit
prodex audit --tail 20 --component profile
prodex doctor --quota
prodex doctor --runtime
prodex doctor --runtime --json
```

If a runtime session looks stalled, inspect the latest proxy log:

```bash
prodex doctor --runtime
tail -n 200 "$(cat /tmp/prodex-runtime-latest.path)"
```

That pointer path stays in `/tmp` only when the runtime log directory is still on the default setting. If you override the log directory, use `prodex doctor --runtime --json` to find the active `log_path` and live broker metrics.

Use `prodex cleanup` when you want to clear stale local runtime logs, temp login homes, transient runtime cache files and stale root temp files inside `.prodex`, prune Codex and Claude chat history older than one week, collapse duplicate profiles with the same account email into one surviving profile, clear dead broker artifacts, and remove old orphaned managed profile homes that are no longer tracked.

Use `prodex audit` when you want to inspect the local append-only audit log. It supports `--tail`, `--component`, `--action`, `--outcome`, and `--json`.

For managed local deployments, you can pin runtime logging and proxy tuning in `~/.prodex/policy.toml`:

```toml
version = 1

[runtime]
log_format = "json"
log_dir = "runtime-logs"

[secrets]
backend = "file"
# keyring_service = "prodex"

[runtime_proxy]
worker_count = 16
active_request_limit = 128
responses_active_limit = 96
```

Environment variables still override `policy.toml`.

## Enterprise Notes

Prodex is still a local-first tool, even after the current enterprise hardening work.

Current hardening includes:

- a secret-management abstraction for `auth.json` and export bundles, plus global secret-backend selection through policy or environment
- a stable broker metrics JSON endpoint at `/__prodex/runtime/metrics`
- a Prometheus broker metrics endpoint at `/__prodex/runtime/metrics/prometheus`
- `prodex info` and `prodex doctor --runtime --json` surfacing live metrics targets and the selected secret backend
- enterprise audit logging for profile, rotation, and admin events, separate from runtime session output and discoverable through `prodex info` or `prodex doctor --runtime --json`
- `prodex audit` for browsing the local append-only audit log without touching runtime proxy behavior

Known gaps today:

- local `auth.json` remains the compatibility source of truth for current Codex flows even when a non-file backend is selected
- no keychain, Vault, or KMS-backed secret storage implementation yet
- audit logs follow the resolved runtime log directory by default, or `PRODEX_AUDIT_LOG_DIR` when set
- no RBAC, SSO, SCIM, or central admin plane
- runtime observability is still centered on local logs plus `doctor --runtime --json`
- the profile pool is still owned per host, not by a shared service
- runtime-store modularization is still underway, so the state layer should be treated as an internal boundary rather than a stable integration surface

Useful markers:

- `runtime_proxy_queue_overloaded`
- `runtime_proxy_active_limit_reached`
- `runtime_proxy_lane_limit_reached`
- `profile_inflight_saturated`
- `profile_retry_backoff`
- `profile_transport_backoff`
- `profile_health`
- `precommit_budget_exhausted`
- `first_upstream_chunk`
- `first_local_chunk`
- `stream_read_error`
