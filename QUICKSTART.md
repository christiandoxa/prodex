# Quick Start

One Prodex profile pool for OpenAI-backed routing, plus direct pass-through when a profile selects a different upstream `model_provider`.

Use `prodex` for Codex CLI, `prodex caveman` for Caveman-mode Codex, `prodex claude` for Claude Code, and add the `mem` prefix when you want to reuse an existing Claude-Mem install with either front end. OpenAI/Codex profiles use Prodex quota-aware routing. `prodex quota` also supports Google Gemini OAuth profiles and imported Copilot accounts through their provider quota APIs. Codex CLI 0.124.0 and newer versions support Amazon Bedrock and OpenAI-compatible custom providers through `model_provider`; when a selected profile sets a non-OpenAI value such as `amazon-bedrock`, `prodex run` and `prodex caveman` launch Codex directly without quota preflight or the local auto-rotate proxy, `prodex quota` is unavailable for that profile, and `prodex claude` is unsupported.

For contributors: this is a Cargo workspace. `src/main.rs` is the binary entrypoint, `src/lib.rs` is a compatibility shim, application orchestration lives under `crates/prodex-app/`, and reusable leaf crates live under `crates/`.

Contributor testing guidance lives in [docs/testing.md](./docs/testing.md), including the fast/serial split and runtime parallel-safety assumptions.

## Requirements

- An OpenAI account and at least one logged-in Prodex profile for quota-aware routing and auto-rotate
- Codex CLI 0.124.0 or newer if you want to use Amazon Bedrock or another custom `model_provider`
- Codex CLI if you want to use `prodex`
- Claude Code (`claude`) if you want to use `prodex claude`
- Optional: `claude-mem` if you want `prodex caveman mem`, `prodex claude mem`, or `prodex claude caveman mem`
- Optional: RTK (`rtk-ai/rtk`) if you want `prodex caveman mem rtk` or default `prodex super` RTK shell-command guidance
- Optional: `sqz-mcp`, `token-savior`, and `claw-compactor` if you want the matching `prodex sqz`, `prodex tokensavior`, or `prodex clawcompactor` optimizer overlays

If you install `@christiandoxa/prodex` from npm, the runtime dependency `@openai/codex@latest` is installed for you at install or update time. Claude Code is still a separate CLI and should already be installed when you use `prodex claude`.

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

Or install from a source checkout:

```bash
cargo install --path .
```

## Update

Check your installed version first:

```bash
prodex --version
```

The current local version in this repo is `0.131.0`:

```bash
npm install -g @christiandoxa/prodex@0.131.0
```

Dependency status in this repo:

- The npm runtime dependency follows `@openai/codex@latest` in the workspace package manifest
- Source installs still use whatever `codex` binary is on your `PATH`
- `prodex update` passes through to `codex update` directly without profile selection, quota preflight, or the local runtime proxy
- Run `cargo update` whenever dependency metadata changes so the workspace lockfile stays in sync
- Versioned npm install snippets in this guide and `README.md` are synced from `Cargo.toml`

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
prodex login --with-google
printf '%s\n' "$OPENAI_API_KEY" | prodex login --with-api-key --base-url http://localhost:11434/v1
```

Interactive `prodex login` asks for ChatGPT browser login, device-code login, API-key login, or Google sign-in for Gemini before opening any browser.

If you want a fixed profile name first:

```bash
prodex profile add second
prodex login --profile second
```

Managed Prodex profiles keep `auth.json` isolated per profile, but Codex-owned history, session, and environment state use the native Codex home by default (`~/.codex` on Unix-like systems). That keeps `history.jsonl`, `sessions`, and `environments.toml` aligned with direct Codex, so logout or account switching does not hide prior chats.

Older Prodex shared state from `$PRODEX_HOME/.codex` is merged into the native Codex home on the next managed-profile launch. Set `PRODEX_SHARED_CODEX_HOME` only when you intentionally want a different shared Codex root.

## 2. Inspect the pool

```bash
prodex profile list
prodex profile export
prodex profile import copilot
prodex quota --all
prodex quota --all --auth no-auth --once
prodex session list
prodex info
```

`prodex quota --all` refreshes live by default. Use `--once` when you want a single snapshot:

```bash
prodex quota --all --once
```

Use `prodex session list` to inspect shared Codex parent sessions, or `prodex session current` to show parent sessions started from the current directory. Add `--include-subagents` only when you explicitly need spawned agent sessions for diagnostics.

`prodex info` includes the effective runtime worker, admission, websocket, lane, and inflight tuning values after environment, policy, and default resolution.
For the full policy key reference, see [docs/runtime-policy.md](./docs/runtime-policy.md).

Backup or move profiles:

```bash
prodex profile export
prodex profile export backup.json
prodex profile import backup.json
prodex profile remove --all
```

`prodex profile export` exports all configured profiles by default and asks whether to password-protect the bundle, defaulting to protected. In non-interactive use, pass `--password-protect` with `PRODEX_PROFILE_EXPORT_PASSWORD` set, or pass `--no-password` to explicitly write an unencrypted bundle.

`prodex profile import copilot` records the logged-in Copilot account and provider endpoint in Prodex while leaving the token in Copilot's own keychain/config storage. The imported profile appears in the pool and export/import flow. Plain `prodex run` still targets OpenAI/Codex profiles, while `prodex s --provider gemini` can use a Google sign-in profile and `prodex quota` can inspect Copilot and Gemini provider quota data through their provider APIs. Profiles whose `config.toml` sets a non-OpenAI `model_provider` are not quota-compatible in Prodex.

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

New Codex top-level subcommands stay on this managed path by default. For example, `prodex remote-control` is treated as `prodex run remote-control` unless Prodex explicitly adds its own command with that name.

Codex CLI 0.124.0 added first-class Amazon Bedrock and OpenAI-compatible custom provider support. Configure Bedrock or another provider in the selected profile's Codex `config.toml`, for example with `model_provider = "amazon-bedrock"`.

If the selected profile sets `model_provider` to a non-OpenAI backend, Prodex skips quota preflight and launches Codex directly without the local runtime proxy. Bedrock quota, credentials, regions, and provider errors are handled by Codex and the upstream provider, not by Prodex.

## 4. Run Codex with `prodex caveman`

```bash
prodex caveman
prodex caveman mem
prodex caveman mem rtk
prodex rtk
prodex sqz
prodex tokensavior
prodex clawcompactor
prodex super --url http://127.0.0.1:8131
prodex super --url http://127.0.0.1:8131 --dry-run
prodex caveman --profile second
prodex caveman exec "review this repo in caveman mode"
```

Use this path when you want Codex itself as the front end but want Caveman mode preloaded from the upstream Caveman plugin. Prodex launches Caveman from a temporary overlay `CODEX_HOME` so the base profile home stays unchanged after the session exits.

If the selected profile sets `model_provider` to a non-OpenAI backend, Prodex skips quota preflight and launches Caveman directly without the local runtime proxy.

Use `prodex caveman mem` when you also want an existing Claude-Mem Codex install to follow the selected Prodex session path instead of watching only the default `~/.codex/sessions` tree.
Add optimizer prefixes before Codex args to inject launch overlays into the temporary Codex home: `mem`, `rtk`, `sqz`, `tokensavior`, `clawcompactor`, or `presidio`. Top-level shortcuts such as `prodex rtk` and `prodex sqz` map to `prodex caveman <prefix>`. RTK is an external binary from `rtk-ai/rtk`; install it separately if `rtk gain` is unavailable.
Mem mode uses a slim Codex transcript schema by default so recall stays lower-token; use `prodex super --mem-super-slim` to store prompt summaries/references instead of full prompt bodies, or `prodex super --mem-full` when you need full assistant/tool transcript capture.
`prodex super` and its `prodex s` alias also enable RTK guidance and Smart Context Autopilot through a dedicated runtime proxy for OpenAI/Codex providers, then ask whether to enable Presidio redaction. Empty input or `n` is equivalent to `prodex caveman mem rtk sqz tokensavior clawcompactor --full-access`; `y` is equivalent to `prodex caveman mem rtk sqz tokensavior clawcompactor presidio --full-access`. Use `--presidio` or `--no-presidio` to make that choice non-interactive. When enabled, Presidio redacts UTF-8 HTTP request bodies and WebSocket text frames through local Presidio before forwarding them upstream. The proxy preserves exact continuation behavior, then safely reduces token load with adaptive budgeting, artifact-backed tool outputs, duplicate/blob suppression, stable cache-friendly context framing, and critical-signal self-checks.
Super's optimization stack is local and deterministic by default: Caveman, Claude-Mem, an overlay `rtk` wrapper plus RTK auto-wrappers for common noisy commands when RTK is installed, auto-registered `sqz-mcp` and `token-savior` MCP servers when those binaries are already on `PATH` or in a managed `prodex-optimizers` checkout, overlay `sqz`/`claw-compactor` wrappers when discoverable, and a trusted one-shot `prodex-claw-compactor-sessionstart` SessionStart benchmark probe when Claw-Compactor is available. The probe delegates to `prodex-claw-compactor-auto "$(pwd)"` and uses a marker under `CODEX_HOME`, so Codex conversation restarts do not replay it. If the directory has no Markdown memory files, Prodex benchmarks a temporary shadow workspace with a generated `MEMORY.md` and does not modify the original directory.
For token-savior, prefer an isolated stable-Python venv at `~/.local/share/prodex-optimizers/token-savior/.venv`; Prodex prefers that managed venv over a global `PATH` binary to avoid experimental Python dependency breakage.
Prodex passes token-savior cache and stats paths under `PRODEX_HOME` (default `~/.prodex`) so compatible token-savior versions keep generated state out of worktrees.
RTK handles upstream/input command output before it enters the context window through visible `rtk <cmd>` commands, with overlay auto-wrappers as a safety fallback. Auto-wrappers are only a backstop; write `rtk <cmd>` explicitly when you want the TUI/transcript to show RTK usage. SQZ handles downstream/context reuse through the auto-registered `prodex-sqz` MCP server when `sqz-mcp` is available.
Managed optimizer checkouts are discovered from `PRODEX_OPTIMIZERS_HOME`, `$XDG_DATA_HOME/prodex-optimizers`, then `~/.local/share/prodex-optimizers`.

Use DeepSeek with the Codex/Super front end:

```bash
DEEPSEEK_API_KEY=... prodex s --provider deepseek --model deepseek-v4-pro
```

`--api-key` is also accepted, but the environment variable avoids shell-history/process-list exposure. This path starts a local Responses-to-DeepSeek adapter, injects a one-model Codex catalog for the selected DeepSeek model, skips OpenAI quota/rotation, and keeps the Super optional tools working as local Codex overlays. `/model` stays on that DeepSeek model and offers `high`/`xhigh` effort choices. Remote compact is not implemented for the adapter yet.

Use Gemini with Google sign-in or an API key:

```bash
prodex login --with-google
prodex s --provider gemini
GEMINI_API_KEY=... prodex s --provider gemini --model gemini-2.5-pro
```

When no API key is supplied, the Gemini path uses the Google OAuth profile from login. With `--api-key`, `GEMINI_API_KEY`, or `GOOGLE_API_KEY`, it uses the public Gemini API. `prodex quota` reads the same Google OAuth profile and asks Code Assist for `retrieveUserQuota` bucket data. Super optional tools stay active because they are local Codex overlays.

Use `prodex super --url http://127.0.0.1:8131` to keep Super mode but route Codex directly to a local OpenAI-compatible server such as `llama-server`. Prodex appends `/v1` when the URL has no path, disables non-function native tools that local servers commonly reject, advertises a conservative 16k local context window, and defaults the local model id to `unsloth/qwen3.5-35b-a3b`; override it with `--model`. Use `--context-window` and `--auto-compact-token-limit` if your local server is configured larger. See [LOCAL.md](./LOCAL.md) for self-hosted model setup and testing.

Use `--dry-run` on `prodex run`, `prodex caveman`, or `prodex super` when you want to inspect resolved provider/model choices, proxy args, and launch env without starting Codex. Secret-looking values are redacted.

Prodex uses system and environment proxy settings for upstream OpenAI quota/auth/runtime HTTP by default, and runtime WebSocket upstream connections use HTTP CONNECT when `HTTPS_PROXY`/`https_proxy` is configured. Localhost broker traffic bypasses proxies with `NO_PROXY` entries for `127.0.0.1`, `localhost`, and `::1`. Add `--no-proxy` to `prodex run`, `prodex caveman`, `prodex super`, or `prodex claude` only when you explicitly want Prodex upstream requests to ignore proxy settings.

## 5. Run Claude Code with `prodex claude`

```bash
prodex claude -- -p "summarize this repo"
prodex claude mem -- -p "recall past work on this repo"
prodex claude caveman
prodex claude caveman mem
prodex claude caveman -- -p "summarize this repo briefly"
prodex claude --profile second -- -p --output-format json "show the latest diff"
```

Use this path when you want Claude Code to be the front end while Prodex routes requests through your OpenAI-backed profile pool.

This path requires the default OpenAI/Codex provider. Profiles whose `config.toml` sets a non-OpenAI `model_provider`, including `amazon-bedrock`, are not supported by `prodex claude`.

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
prodex doctor --install
prodex audit
prodex audit --tail 20 --component profile
prodex context audit
prodex context compress ~/.codex/AGENTS.md --dry-run
prodex doctor --quota
prodex doctor --runtime
prodex doctor --runtime --json
prodex doctor --bundle ./prodex-doctor.json --redacted
prodex setup --dry-run
prodex capability list
```

If you see `409 stale_continuation`, Prodex found continuation state for the request but could not safely replay it as a fresh turn on a different profile. That is deliberate: the missing or stale binding may still belong to a specific profile, session, or tool-output chain, and replaying it elsewhere can break the conversation. Start a new prompt, or return to the same session/profile if the original continuation is still available.

If a runtime session looks stalled, inspect the latest proxy log:

```bash
prodex doctor --runtime
prodex doctor --runtime --json
tail -n 200 "$(prodex doctor --runtime --json | jq -r '.log_path')"
```

The default runtime log directory is the OS temp directory, usually `/tmp` on Linux, but `PRODEX_RUNTIME_LOG_DIR` or `runtime.log_dir` in `policy.toml` can override it.
Use `prodex doctor --runtime --json` to find the active `log_path`, resolved `runtime_logs.directory`, and live broker metrics before tailing files.
Use `prodex doctor --bundle ./prodex-doctor.json --redacted` when sharing diagnostics; the bundle includes version, policy/config, profile, and runtime-log summaries without auth tokens or headers.

Prodex also schedules non-blocking automatic housekeeping for stale runtime logs, temp login homes, stale root temp files, and dead broker artifacts. Use `prodex cleanup` when you want a manual cleanup that also clears transient runtime cache files, collapses duplicate profiles that point at the same OpenAI workspace identity into one surviving profile, and removes old orphaned managed profile homes that are no longer tracked. Orphaned managed profile homes use a conservative 7-day threshold by default; override that explicitly with `prodex cleanup --older-than 1d` or `prodex cleanup --aggressive` when you want faster reclaim. Codex and Claude chat histories are left to the upstream runtimes.

Use `prodex audit` when you want to inspect the local append-only audit log. It supports `--tail`, `--component`, `--action`, `--outcome`, and `--json`.

Use `prodex context audit` when shared Codex memory, rule, skill, or AGENTS files feel too token-heavy. Use `prodex context compress PATH --dry-run` first; compression is local and deterministic, skips backups, only edits Markdown/text prose files, and writes an `.original.md` backup before replacement.

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
Use `prodex info` to inspect the resulting effective runtime tuning values.
See [docs/runtime-policy.md](./docs/runtime-policy.md) for all `runtime` and `runtime_proxy` keys, env overrides, defaults, and meanings.

## Enterprise Notes

Prodex is still a local-first tool, even after the current enterprise hardening work.

Current hardening includes:

- a secret-management abstraction for `auth.json` and export bundles, plus global secret-backend selection through policy or environment
- a stable broker metrics JSON endpoint at `/__prodex/runtime/metrics`
- a Prometheus broker metrics endpoint at `/__prodex/runtime/metrics/prometheus`
- `prodex info` and `prodex doctor --runtime --json` surfacing live metrics targets and the selected secret backend
- `prodex doctor --bundle PATH --redacted` for shareable local diagnostics without stored auth tokens
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
