# Quick Start

One Prodex profile pool for OpenAI-backed routing, plus runtime provider bridges for Gemini, Anthropic, Copilot, Kiro, DeepSeek, and local OpenAI-compatible servers.

Use `prodex` for Codex CLI, `prodex caveman` for Caveman-mode Codex, and `prodex claude` for Claude Code. OpenAI/Codex profiles use Prodex quota-aware routing. `prodex s gemini`, `prodex s deepseek`, or `prodex s --provider gemini|anthropic|copilot|kiro|deepseek` keeps the Codex/Super front end while routing to those provider backends. `prodex quota` supports Google Gemini OAuth profiles, Antigravity CLI quota snapshots, Anthropic OAuth profiles, imported Copilot accounts, imported Kiro accounts, DeepSeek API-key balances, local OpenAI-compatible health checks, and custom provider metadata snapshots. Codex CLI 0.124.0 and newer versions support Amazon Bedrock and OpenAI-compatible custom providers through `model_provider`; when a selected profile sets a non-OpenAI value such as `amazon-bedrock`, `prodex run` and `prodex caveman` launch Codex directly without quota preflight or the local auto-rotate proxy, and `prodex claude` is unsupported.

For contributors: this is a Cargo workspace. `src/main.rs` is the binary entrypoint, `src/lib.rs` is a compatibility shim, application orchestration lives under `crates/prodex-app/`, and reusable leaf crates live under `crates/`.

Contributor testing guidance lives in [docs/testing.md](./docs/testing.md), including the fast/serial split and runtime parallel-safety assumptions.

## Requirements

- An OpenAI account and at least one logged-in Prodex profile for quota-aware routing; use multiple profiles when you want auto-rotate
- Codex CLI 0.124.0 or newer if you want to use Amazon Bedrock or another custom `model_provider`
- Codex CLI if you want to use `prodex`
- Claude Code (`claude`) if you want to use `prodex claude`
- Optional: RTK (`rtk-ai/rtk`) if you want `prodex rtk` or default `prodex super` RTK shell-command guidance
- Optional: `sqz-mcp`, `token-savior`, `codebase-memory-mcp`, `claw-compactor`, and a Ponytail checkout if you want the matching `prodex sqz`, `prodex tokensavior`, `prodex clawcompactor`, or `prodex ponytail` optimizer tools

If you install `@christiandoxa/prodex` from npm, Prodex uses its bundled `@openai/codex@latest` dependency by default so a broken or architecture-mismatched global `codex` on `PATH` does not affect `prodex run`. If the bundled native Codex optional package is missing or not executable, Prodex falls back to an executable external `codex` on `PATH` and prints a notice before launch, including nvm-managed global Codex installs when the active nvm version's `bin` directory is on `PATH`. If neither bundled nor external Codex is usable, set `PRODEX_CODEX_AUTO_INSTALL=1` to let Prodex run `npm install -g @openai/codex@latest` once before retrying PATH resolution. To deliberately use an external Codex CLI, set `PRODEX_CODEX_BIN=/path/to/codex` or `PRODEX_CODEX_RESOLUTION=external`. Claude Code is still a separate CLI and should already be installed when you use `prodex claude`.

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

The current local version in this repo is `0.254.0`:

```bash
npm install -g @christiandoxa/prodex@0.254.0
```

Dependency status in this repo:

- The npm runtime dependency follows `@openai/codex@latest` in the workspace package manifest and is used by default; external Codex is opt-in through `PRODEX_CODEX_BIN` or `PRODEX_CODEX_RESOLUTION=external`, and one-shot npm repair is opt-in through `PRODEX_CODEX_AUTO_INSTALL=1`
- Source installs still use whatever `codex` binary is on your `PATH`
- Packaged Codex runtime resources, including the Codex 0.136.0 and newer bundled zsh runtime helper, stay owned by the Codex package; Prodex does not override `zsh_path`
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

If you already use Claude Code, GitHub Copilot CLI, or Kiro CLI on this machine and want Prodex to track those provider identities:

```bash
prodex profile import claude
prodex profile import copilot
prodex profile import kiro
prodex profile import copilot --name copilot-main --activate
prodex profile import kiro --name kiro-main --activate
```

Or create a profile through the normal Codex login flow:

```bash
prodex login
prodex login --device-auth
prodex login --with-google
prodex login --with-claude
prodex login --with-antigravity
printf '%s\n' "$OPENAI_API_KEY" | prodex login --with-api-key --base-url http://localhost:11434/v1
```

Interactive `prodex login` asks for ChatGPT browser login, device-code login, API-key login, Google sign-in for Gemini, Claude sign-in through Claude Code, or Antigravity CLI sign-in through `agy auth login` before opening any browser. Antigravity login is global to the `agy` CLI and does not create a Prodex profile.

If you want a fixed profile name first:

```bash
prodex profile add second
prodex login --profile second
```

Managed Prodex profiles keep `auth.json` isolated per profile, but Codex-owned history, session, environment, managed config, MCP OAuth fallback credentials, plugin/app-server, remote-control enrollment, and memory database state use the native Codex home by default (`~/.codex` on Unix-like systems). That keeps `history.jsonl`, `sessions`, `archived_sessions`, `managed_config.toml`, `environments.toml`, `.credentials.json`, plugin cache/state, and Codex SQLite files including `state_*` and `memories_*` aligned with direct Codex, so logout or account switching does not hide prior chats.

Codex 0.140.0 defaults CLI auth credentials to the file store, so managed profiles still carry profile-local `auth.json`, including Bedrock API-key auth JSON. MCP OAuth defaults to Codex `auto`; file fallback lives in shared `.credentials.json`, while OS keyring credentials stay Codex/OS-owned and are not exported by Prodex.

Codex cloud-managed config bundle caches are identity/account scoped and remain profile-local. System-level Codex requirements and managed config files remain owned by upstream Codex and the operating system.

Older Prodex shared state from `$PRODEX_HOME/.codex` is merged into the native Codex home on the next managed-profile launch. Set `PRODEX_SHARED_CODEX_HOME` only when you intentionally want a different shared Codex root.

Codex file-based profiles selected by `--profile` remain Codex-owned shared config. Prodex does not re-enable legacy Codex `[profiles.*]` behavior; Prodex account selection stays in Prodex profile metadata.

## 2. Inspect the pool

```bash
prodex profile list
prodex profile export
prodex profile import claude
prodex profile import copilot
prodex profile import kiro
prodex quota --all
prodex quota --all --auth no-auth --once
prodex quota --all --detail --provider openai
prodex quota --all --provider deepseek --once
prodex quota --all --provider local --base-url http://127.0.0.1:8131/v1 --once
prodex redeem main
prodex dashboard
prodex session list
prodex info
```

`prodex quota --all` refreshes live by default. Use `--once` when you want a single snapshot:

```bash
prodex quota --all --once
```

In the live `prodex quota --all --detail` view, press `f` to cycle provider filters: `all`, `openai`, `gemini`, `anthropic`, `copilot`, `kiro`, `deepseek`, `local`. Add `--provider openai`, `--provider gemini`, `--provider anthropic`, `--provider copilot`, `--provider kiro`, `--provider deepseek`, or `--provider local` to start locked to one provider.

For OpenAI/Codex profiles, quota views also show earned rate-limit reset credits when the upstream usage API reports them. Use `prodex redeem <profile>` when you explicitly want to redeem one reset credit on a named profile, even if the 5h and weekly quota windows still have remaining quota. If either quota window resets within 1 hour, Prodex asks before consuming the credit; pass `--yes` to skip that prompt.

`prodex dashboard` starts a localhost browser dashboard, defaulting to `http://127.0.0.1:8765`. It shows configured profiles, active account controls, and live quota usage using the same quota collectors as `prodex quota`. Use `prodex dashboard --port 0` to bind a free port, or `--base-url` to point quota checks at a custom Codex-compatible backend. The dashboard has no password auth, so keep it bound to localhost unless the network is trusted.

Use `prodex session list` to inspect shared Codex sessions, or `prodex session current` to show sessions started from the current directory. Add `--parent-only` when you only want resumable parent sessions.

`prodex info` includes provider route/quota-shape summaries, the runtime proxy contract, and the effective runtime worker, admission, websocket, lane, and inflight tuning values after environment, policy, and default resolution.
For the full policy key reference, see [docs/runtime-policy.md](./docs/runtime-policy.md).

Backup or move profiles:

```bash
prodex profile export
prodex profile export backup.json
prodex profile import backup.json
prodex profile remove --all
```

`prodex profile export` exports all configured profiles by default and asks whether to password-protect the bundle, defaulting to protected. In non-interactive use, pass `--password-protect` with `PRODEX_PROFILE_EXPORT_PASSWORD` set, or pass `--no-password` to explicitly write an unencrypted bundle.

`prodex profile import claude` imports the current Claude Code OAuth credentials from `CLAUDE_CONFIG_DIR` or `~/.claude` into a Prodex-managed Anthropic profile. `prodex profile import copilot` records the logged-in Copilot account and provider endpoint in Prodex while leaving the token in Copilot's own keychain/config storage. `prodex profile import kiro` reads the installed Kiro CLI auth database, snapshots the current auth payload into the managed profile, and refreshes a Kiro model catalog snapshot; override CLI discovery with `PRODEX_KIRO_BIN` when needed. Plain `prodex run` still targets OpenAI/Codex profiles, while `prodex s gemini` can use a Google sign-in profile and `prodex quota` can inspect Copilot, Kiro, Gemini, Antigravity CLI, Anthropic, DeepSeek, local, and custom provider snapshots. Profiles whose `config.toml` sets a non-OpenAI `model_provider` are not OpenAI quota-compatible, but they still render provider metadata in `prodex quota`.

## 3. Run Codex CLI with `prodex`

`prodex` without a subcommand is shorthand for `prodex run`.

```bash
prodex
prodex run
prodex run --profile second
prodex run 019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9
prodex exec "review this repo"
prodex delete 019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9
printf 'context from stdin' | prodex run exec "summarize this"
```

Use this path when you want Codex CLI itself to be the front end. Prodex keeps transport behavior close to direct Codex while handling profile selection, quota preflight, continuation affinity, and safe pre-commit rotation. Prodex does not auto-redeem reset credits by default; add `--auto-redeem` when you want guarded automatic single reset-credit redemption only after the OpenAI/Codex weekly window is exhausted for every profile and the weekly reset is not already imminent. Manual `prodex redeem <profile>` is an explicit one-profile consume request; the upstream backend decides whether it applies or reports nothing-to-reset/no-credit.

Recent Codex runtime feature switches are available on `prodex run`, `prodex caveman`, and `prodex super` as Codex config overrides: `--web-search disabled|cached|indexed|live`, `--rollout-budget-tokens <tokens>`, `--current-time-reminder`, and `--respect-system-proxy` / `--no-respect-system-proxy`. Multi-agent `multiAgentMode` remains an upstream app-server/thread setting; use `prodex app-server` or `prodex run app-server` and pass `none`, `explicitRequestOnly`, or `proactive` through the Codex app-server API. `mcp-server`, `app-server`, and `exec-server` are direct Codex command-server passthroughs by default: Prodex selects profile `CODEX_HOME`, but does not wrap stdio, inject the runtime proxy, rotate accounts inside the protocol, or apply gateway guardrails. `prodex app-server-broker --json` reports the disabled-by-default JSON-RPC broker skeleton; stdio brokering is not enabled yet. Codex plugin catalog commands such as `prodex plugin list` are managed passthrough launches by default.

New Codex top-level subcommands stay on this managed path by default. For example, `prodex remote-control` is treated as `prodex run remote-control` unless Prodex explicitly adds its own command with that name. Codex-owned TUI commands such as `/usage`, `/goal`, `/import`, and `/delete` remain upstream behavior; `prodex delete <session>` passes through to Codex and prunes matching Prodex session affinity metadata after a successful delete.

Codex CLI 0.124.0 added first-class Amazon Bedrock and OpenAI-compatible custom provider support. Configure Bedrock or another provider in the selected profile's Codex `config.toml`, for example with `model_provider = "amazon-bedrock"`.

Codex 0.142.3 includes the upstream Bedrock GPT-5.6 Sol, Terra, and Luna catalog entries. Prodex does not proxy or rewrite those Bedrock launches.

If the selected profile sets `model_provider` to a non-OpenAI backend, Prodex skips quota preflight and launches Codex directly without the local runtime proxy. `prodex quota` still shows the configured provider metadata; Bedrock quota, credentials, regions, and provider errors are handled by Codex and the upstream provider.

## 4. Run Codex with `prodex caveman`

```bash
prodex caveman
prodex rtk
prodex sqz
prodex tokensavior
prodex clawcompactor
prodex ponytail
prodex mem
prodex super --url http://127.0.0.1:8131
prodex super --url http://127.0.0.1:8131 --dry-run
prodex s expose
prodex caveman --profile second
prodex caveman exec "review this repo in caveman mode"
```

Use this path when you want Codex itself as the front end but want Caveman mode preloaded from the upstream Caveman plugin. Prodex launches Codex from a temporary Prodex overlay `CODEX_HOME`, then activates Caveman for that session so the base profile home stays unchanged after the session exits.

If the selected profile sets `model_provider` to a non-OpenAI backend, Prodex skips quota preflight and launches Caveman directly without the local runtime proxy.

Add optimizer prefixes before Codex args to enable session tools in the Prodex overlay: `rtk`, `sqz`, `tokensavior`, `clawcompactor`, `ponytail`, `mem`, or `presidio`. Top-level shortcuts such as `prodex rtk`, `prodex sqz`, `prodex ponytail`, and `prodex mem` map to `prodex caveman <prefix>`. RTK is an external binary from `rtk-ai/rtk`; install it separately if `rtk gain` is unavailable.
`prodex super` and its `prodex s` alias also enable RTK guidance and Smart Context Autopilot through a dedicated runtime proxy for OpenAI/Codex providers, then ask whether to enable Presidio redaction. Empty input or `n` is equivalent to `prodex caveman rtk sqz tokensavior clawcompactor ponytail --full-access`; `y` is equivalent to `prodex caveman rtk sqz tokensavior clawcompactor ponytail presidio --full-access`. Use `--presidio` or `--no-presidio` to make that choice non-interactive. Super then asks whether to enable prodex-memory through managed Mem0 Docker. Empty input or `n` leaves `prodex-memory` disabled; answer `y` or pass `--mem0` to start the managed Mem0 OSS Docker server, route its OpenAI-compatible calls through a session-local Prodex gateway, and avoid Mem0 Cloud or `MEM0_API_KEY`. Use `--no-mem0` to skip that prompt, or the `mem` optimizer prefix for local SQLite memory. Managed Mem0 mode requires Docker Compose; if no upstream provider API key is available, Prodex serves deterministic local embeddings for Mem0 and keeps generation disabled on that internal gateway. When Presidio is enabled and healthy, it redacts UTF-8 HTTP request bodies and WebSocket text frames through local Presidio before forwarding them upstream; service failures follow the configured `fail_mode`. The proxy preserves exact continuation behavior, then safely reduces token load with adaptive budgeting, artifact-backed tool outputs, duplicate/blob suppression, stable cache-friendly context framing, and critical-signal self-checks.
Use `prodex s doctor` to check RTK, SQZ MCP, token-savior, codebase-memory-mcp, claw-compactor, optional built-in prodex-memory, built-in Smart Context, and embedded Caveman/Super assets without launching Codex. Add `--json`, `--strict`, or `--presidio` when you need machine-readable output, non-zero failure on unavailable tools, or Presidio service health checks. `prodex s --dry-run` includes the same optimizer matrix in the launch preview.
Use `prodex log` to show the latest session transcript text plus the latest sent/input and received/output token counts from runtime `token_usage` markers. Use `prodex log stream` to follow transcript and token events live. Use `prodex log upstream` to follow HTTP backend-bound LLM payload snapshots after Prodex processing such as Presidio redaction and Smart Context rewriting; WebSocket request payloads are not logged, and HTTP snapshots are capped at 128 KiB per payload. Add `--json` when you want JSON Lines events.
Use `prodex s expose` to start a browser-accessible terminal session protected by a high-entropy access token. When `cloudflared` is on `PATH`, Prodex creates a Cloudflare quick tunnel and prints the public URL; use `--no-tunnel` for loopback-only access. Closing the browser tab does not stop the PTY, and reopening the same token URL replays recent terminal scrollback while the `prodex s expose` process is still running.
Super's optimization stack is local and deterministic by default: Caveman, Ponytail when a managed checkout exists, a Prodex overlay `rtk` PATH wrapper plus RTK auto-wrappers for common noisy commands when RTK is installed, built-in read-only `prodex-inspect` MCP diagnostics, `sqz-mcp`, `token-savior`, and `codebase-memory-mcp` MCP servers when discoverable, and `sqz`/`claw-compactor` wrapper commands when discoverable. When Claw-Compactor is available, Super also installs a trusted one-shot `prodex-claw-compactor-sessionstart` SessionStart benchmark probe wrapper. The startup probe is disabled by default so Codex launch is not delayed; opt in with `PRODEX_CLAW_SESSIONSTART_TIMEOUT_SECONDS=<seconds>`. When enabled, the probe delegates to `prodex-claw-compactor-auto "$(pwd)"` and uses a marker under `CODEX_HOME`, so Codex conversation restarts do not replay it. If the directory has no Markdown memory files, Prodex benchmarks a temporary shadow workspace with a generated `MEMORY.md` and does not modify the original directory.
For token-savior, prefer an isolated stable-Python venv at `~/.local/share/prodex-optimizers/token-savior/.venv`; Prodex prefers that managed venv over a global `PATH` binary to avoid experimental Python dependency breakage.
Prodex routes optional-tool state under `PRODEX_HOME` (default `~/.prodex`), including SQZ XDG state, token-savior cache/stats, Codebase Memory cache/config, claw-compactor HOME/XDG state, RTK analytics paths, and local memory, so compatible optimizer metadata stays out of worktrees. `prodex-memory` is local Mem0-style memory and does not use Mem0 Cloud or require `MEM0_API_KEY`; the `mem` prefix uses SQLite, and `--mem0` switches Super to the managed Mem0 OSS Docker backend.
Super instructs Codex to use the available local optimizer stack where it fits: visible `rtk <cmd>` for noisy shell output when RTK is installed, `prodex-inspect` for read-only Prodex status/profile/runtime-log diagnostics, `prodex-sqz` for repeated large context and long-session reuse when SQZ is available, `prodex-token-savior` for symbol/navigation work before broad source reads when token-savior is available, `codebase-memory-mcp` for structural codebase graph queries when available, optional `prodex-memory` for durable local preferences/project facts, and `prodex-claw-compactor` or `prodex-claw-compactor-auto` for workspace-level summaries when claw-compactor is available. Presidio remains opt-in through the prompt or `--presidio`.
RTK handles upstream/input command output before it enters the context window through visible `rtk <cmd>` commands when RTK is installed, with overlay auto-wrappers as a safety fallback. Auto-wrappers are only a backstop; write `rtk <cmd>` explicitly when you want the TUI/transcript to show RTK usage. SQZ handles downstream/context reuse through the auto-registered `prodex-sqz` MCP server when `sqz-mcp` is available.
Managed optimizer checkouts are discovered from `PRODEX_OPTIMIZERS_HOME`, `$XDG_DATA_HOME/prodex-optimizers`, then `~/.local/share/prodex-optimizers`. Put Ponytail at `~/.local/share/prodex-optimizers/ponytail` with `git clone https://github.com/DietrichGebert/ponytail.git ~/.local/share/prodex-optimizers/ponytail`. The generated `SUPER_OPTIMIZERS.md` overlay includes an `Available Now` section for the exact tools discovered in that session.

Use DeepSeek with the Codex/Super front end:

```bash
DEEPSEEK_API_KEY=... prodex s deepseek --model deepseek-v4-pro
```

`--api-key` is also accepted, but the environment variable avoids shell-history/process-list exposure. `DEEPSEEK_API_KEYS` may contain comma-, semicolon-, or newline-separated keys for round-robin request rotation and pre-commit retry on auth/quota/rate/temporary failures. This path starts a local Responses-to-DeepSeek adapter, injects a one-model Codex catalog for the selected DeepSeek model, skips OpenAI quota preflight, and keeps available Super optimizer tools as local Prodex overlay additions around Codex. `/model` stays on that DeepSeek model and offers `high`/`xhigh` effort choices. `prodex quota --all --provider deepseek` reads the same `DEEPSEEK_API_KEY(S)` values and fetches DeepSeek `/user/balance`. Remote compact is not implemented for the adapter yet.

The DeepSeek catalog includes `deepseek-v4-pro` and `deepseek-v4-flash`; `deepseek-chat` and `deepseek-reasoner` remain compatibility aliases for existing configs.

DeepSeek support is a translated compatibility path: text, function/MCP/local shell/apply-patch style tools, `tool_choice`, reasoning effort, JSON object mode, stop sequences, sampling, token limits (`max_output_tokens`, `max_tokens`, and `max_completion_tokens`), logprobs, streaming usage, and cache hit/miss usage are mapped to DeepSeek OpenAI Chat where possible. Request `metadata`, `client_metadata`, `prompt_cache_key`, and `prompt_cache_retention` are preserved in local response metadata instead of being forwarded upstream. JSON Schema is degraded to JSON object mode and marked in response metadata. When thinking is enabled, explicit `tool_choice` is omitted for upstream compatibility and recorded in response metadata. Reasoning content, refusal text, annotations, logprobs, and finish reasons are preserved in DeepSeek response metadata. Reasoning summaries are not advertised for DeepSeek, and `reasoning.summary` fails clearly. Web search is not advertised as native; `[deepseek] web_search_mode = "auto"` fails clearly because DeepSeek's OpenAI Chat docs do not document native web-search request fields, `"off"` rejects web-search tools, and `"openai_chat"` explicitly selects best-effort `web_search_options` forwarding with fallback. `PRODEX_DEEPSEEK_WEB_SEARCH_MODE` provides the same setting for gateway/profileless use. The `anthropic` and `function_proxy` modes currently fail clearly until those adapters/backends exist. Image/document/audio/video message content is rejected, deprecated penalty fields are not mapped, and `parallel_tool_calls=false` is rejected because DeepSeek has no equivalent enforcement control. Responses-only controls that DeepSeek cannot honor, including non-empty `include`, `store=false`, background responses, `truncation=auto`, per-message `cache_control`, `text.verbosity`, legacy `functions`/`function_call`, `logit_bias`, and `max_tool_calls`, fail clearly instead of being dropped. Function/custom tools must be named, namespace tools must contain named function entries, MCP toolsets that declare inventories need a server name plus allowed/enabled tools, duplicate translated tool names are rejected, and DeepSeek function names must stay within the upstream name rules. Strict function tools are opt-in beta: set `[deepseek] strict_tools = true` in the selected Codex profile config, or `PRODEX_DEEPSEEK_STRICT_TOOLS=1` for gateway/profileless use. Prodex then routes DeepSeek Responses rewrites through `https://api.deepseek.com/beta` unless `[deepseek] beta_base_url = "..."` or `PRODEX_DEEPSEEK_BETA_BASE_URL` overrides it, sets each function tool `strict: true`, and rejects unsupported strict schema keywords instead of silently dropping them. DeepSeek beta prefix completion and FIM are not enabled yet; `prefix`, `prompt`, and `suffix` completion-style requests fail fast.

Troubleshooting: JSON mode still needs JSON-oriented task instructions, too many tools or malformed tool declarations fail before DeepSeek sees the request, strict tool schema errors point at unsupported beta-schema keywords or required/additionalProperties constraints, and `openai_chat` web-search 400s mean that best-effort forwarding shape is not accepted upstream for that request.

Quick compatibility: text/streaming/usage/reasoning/tools are translated through DeepSeek OpenAI Chat; JSON Schema degrades to JSON object; strict tools are beta opt-in; web search is explicit mode-gated; images/documents/audio/video, prefix completion, FIM, and remote compact are unsupported on the current `/responses` adapter unless noted above.

Use Anthropic with the Codex/Super front end:

```bash
prodex login --with-claude
prodex s --provider anthropic --model claude-sonnet-4-6
ANTHROPIC_API_KEY=... prodex s --provider anthropic --model claude-sonnet-4-6
```

Without `--api-key`, this path uses the Anthropic profile from `prodex login --with-claude` or `prodex profile import claude`. API-key mode starts the same local Responses-to-Anthropic adapter and forwards through Anthropic's OpenAI-compatible chat API. Use `ANTHROPIC_API_KEYS` with comma-, semicolon-, or newline-separated keys for round-robin request rotation and pre-commit retry on auth/quota/rate/temporary failures. `prodex quota --all --provider anthropic` shows OAuth readiness; set `ANTHROPIC_ADMIN_KEY` to include Anthropic Admin rate-limit groups.

Use GitHub Copilot with the Codex/Super front end:

```bash
prodex profile import copilot
prodex s --provider copilot --model gpt-5.3-codex
```

Without `--api-key`, Prodex uses imported Copilot CLI profiles, resolves the stored Copilot OAuth token before launch, refreshes the Copilot model catalog, can rotate fresh native Responses requests across multiple eligible profiles, and keeps `previous_response_id` continuations on the owning profile. `GITHUB_COPILOT_API_KEY` or `GITHUB_COPILOT_API_KEYS` is also accepted when you already have a usable Copilot API bearer token; plural keys may be comma-, semicolon-, or newline-separated and can rotate before commit on auth/quota/rate/temporary failures.

Use Kiro with either the Codex/Super front end or the native Kiro CLI:

```bash
prodex profile import kiro
prodex s --provider kiro --model claude-sonnet-4.5
prodex super --cli kiro --profile kiro-main
```

`prodex profile import kiro` reads the installed Kiro CLI auth database, snapshots the current credential payload into the managed profile, and refreshes a Kiro model catalog snapshot for later routing. `--provider kiro` keeps the Codex front end and routes through Prodex's Kiro adapter; `--cli kiro` launches the imported Kiro CLI snapshot directly. Use `PRODEX_KIRO_BIN` if the installed Kiro launcher is not on `PATH`.

Use Gemini with Google sign-in or an API key:

```bash
prodex login --with-google
prodex s gemini
prodex s gemini --cli gemini
prodex s gemini --cli agy
GEMINI_API_KEY=... prodex s gemini --model gemini-2.5-pro
```

When no API key is supplied, the Gemini path uses the Google OAuth profile from `prodex login --with-google` or the interactive Google sign-in choice, then routes through Code Assist. Google login verifies Code Assist readiness before creating or updating the profile, and may open a second browser page if Google requires account verification. With `--api-key`, `GEMINI_API_KEY(S)`, or `GOOGLE_API_KEY(S)`, Prodex converts Codex Responses requests to Chat Completions and uses Google's documented `/v1beta/openai/chat/completions` endpoint with Bearer authentication. Streaming, function calls, continuations, and Gemini reasoning efforts are converted back into Codex Responses semantics. Plural key env vars may be comma-, semicolon-, or newline-separated and can rotate before commit on auth/quota/rate/temporary failures. OAuth sessions keep fresh Gemini requests sticky to the previous successful profile by default for smoother Codex-style continuity; set `PRODEX_GEMINI_STICKY_FRESH_OAUTH=0` to restore pure fresh-request round robin. `prodex quota` reads the same Google OAuth profile and asks Code Assist for `retrieveUserQuota` bucket data. Available Super optimizer tools remain local Prodex overlay additions around Codex.

Add `--cli gemini` to launch the native Gemini CLI with its tools in YOLO mode through Prodex OAuth profile routing. This native path currently requires Google OAuth and does not accept `--api-key`; `PRODEX_GEMINI_BIN` overrides the executable.

Add `--cli agy` to launch Antigravity CLI with `--dangerously-skip-permissions`. Antigravity owns its keyring/Google Sign-In authentication, so Prodex account rotation and Presidio proxying are unavailable for this native path. `PRODEX_AGY_BIN` overrides the executable.

**Feature Parity Mapping (Codex to Gemini):**

- **Tools:** Codex/MCP tool schemas are translated natively to Gemini function declarations (`tools[0].functionDeclarations`).
- **Memory:** Gemini `MEMORY.md` and `INBOX.md` files are loaded and prepended to the context.
- **Settings and extensions:** `mcpServers`, `commands/*.toml`, `skills/*/SKILL.md`, and `agents/*.md` from Gemini CLI are projected into Codex config, hooks, prompts, skills, and agents before launch.
- **Live Mode:** The Gemini Live websocket bridge remains for compatible callers and adapter tests, but Codex 0.140.0 removed upstream TUI voice controls.
- **Context Window:** Prodex handles context limits dynamically, matching Gemini's 1M-token default (`GEMINI_DEFAULT_CONTEXT_WINDOW`), allowing large file analysis.
- **Compaction:** Remote compaction maps to Gemini's semantic chat-compression or a deterministic local summary as fallback.

Gemini memory is loaded by default from `~/.gemini/GEMINI.md`, project `GEMINI.md` files, `.gemini/memory/MEMORY.md`, and `.gemini/memory/INBOX.md`; opt out with `PRODEX_GEMINI_DISABLE_MEMORY=1` or request metadata `gemini_load_memory=false`. Gemini system-defaults, global, ancestor project, cwd-local, and system override settings plus extensions are projected into Codex before launch, honoring `GEMINI_CLI_HOME` and Gemini CLI system settings env vars: `mcpServers` become Codex MCP config, hooks go through `/hooks` review, `commands/*.toml` become custom prompts, `skills/*/SKILL.md` become Codex skills, and `agents/*.md` become Codex custom agents. Generated prompts and helper scripts cover refresh, memory, and checkpoint create/restore workflows. Use `PRODEX_GEMINI_EXTENSIONS=none` to disable extension loading or `PRODEX_GEMINI_DISABLE_CLI_COMPAT=1` to skip launch-time surface projection.

For Gemini adapter checks, run `npm run test:gemini-schema` after translation/schema changes and `PRODEX_LIVE_GEMINI=1 npm run test:gemini-live` for a credentialed live smoke. Add `PRODEX_LIVE_GEMINI_EXTENDED=1` to cover exact command output, file edits, `apply_patch`, reference-repo clone/inspection, optional-tool update discipline, compact, and explicit `exec resume`; add `PRODEX_LIVE_GEMINI_MCP=1` or `PRODEX_LIVE_GEMINI_MULTIMODAL=1` when that machine should also test MCP or image input.

Use `prodex super --url http://127.0.0.1:8131` to keep Super mode but route Codex directly to a local OpenAI-compatible server such as `llama-server`. Prodex appends `/v1` when the URL has no path, disables non-function native tools that local servers commonly reject, advertises a conservative 16k local context window, and defaults the local model id to `unsloth/qwen3.5-35b-a3b`; override it with `--model`. Use `--context-window` and `--auto-compact-token-limit` if your local server is configured larger. Check local reachability with `prodex quota --all --provider local --base-url http://127.0.0.1:8131/v1 --once`. See [LOCAL.md](./LOCAL.md) for self-hosted model setup and testing.

Use `--dry-run` on `prodex run`, `prodex caveman`, or `prodex super` when you want to inspect resolved provider/model choices, proxy args, and launch env without starting Codex. Secret-looking values are redacted.

Prodex uses system and environment proxy settings for upstream OpenAI quota/auth/runtime HTTP by default, and runtime WebSocket upstream connections use HTTP CONNECT when `HTTPS_PROXY`/`https_proxy` is configured. Localhost broker traffic bypasses proxies with `NO_PROXY` entries for `127.0.0.1`, `localhost`, and `::1`. Add `--no-proxy` to `prodex run`, `prodex caveman`, `prodex super`, or `prodex claude` only when you explicitly want Prodex upstream requests to ignore proxy settings.

## 5. Run Claude Code with `prodex claude`

```bash
prodex claude -- -p "summarize this repo"
prodex claude caveman
prodex claude caveman -- -p "summarize this repo briefly"
prodex claude --profile second -- -p --output-format json "show the latest diff"
```

Use this path when you want Claude Code to be the front end while Prodex routes requests through your OpenAI-backed profile pool.

This path requires the default OpenAI/Codex provider. Profiles whose `config.toml` sets a non-OpenAI `model_provider`, including `amazon-bedrock`, are not supported by `prodex claude`.

Use `prodex claude caveman` when you want the same Claude path but with the upstream Caveman plugin loaded through Claude's session-local `--plugin-dir` support. Prodex keeps the plugin bundle stable under `.prodex`, and the adapted Caveman hooks read and write the Prodex-managed `CLAUDE_CONFIG_DIR` instead of your global `~/.claude`.

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
prodex context export 019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9
prodex context compress ~/.codex/AGENTS.md --dry-run
prodex doctor --quota
prodex doctor --runtime
prodex doctor --runtime --json
prodex doctor --bundle ./prodex-doctor.json --redacted
prodex setup --dry-run
prodex capability list
prodex gateway --provider gemini
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
For suspected upstream Codex issues, also run `codex doctor --json` from the same shell. Codex 0.135.0 and newer includes app-server version and thread-inventory diagnostics that Prodex's runtime doctor does not duplicate.

Prodex also schedules non-blocking automatic housekeeping for stale runtime logs, temp login homes, stale root temp files, and dead broker artifacts. Use `prodex cleanup` when you want a manual cleanup that also clears transient runtime cache files, collapses duplicate profiles that point at the same OpenAI workspace identity into one surviving profile, and removes old orphaned managed profile homes that are no longer tracked. Orphaned managed profile homes use a conservative 7-day threshold by default; override that explicitly with `prodex cleanup --older-than 1d` or `prodex cleanup --aggressive` when you want faster reclaim. Codex and Claude chat histories are left to the upstream runtimes.

Use `prodex audit` when you want to inspect the local append-only audit log. It supports `--tail`, `--component`, `--action`, `--outcome`, and `--json`.

Use `prodex context audit` when shared Codex memory, rule, skill, or AGENTS files feel too token-heavy. Use `prodex context export SESSION_ID` to dump a shared Codex session into `./context_<session-id>.md` by default, or pass a custom output path. Use `prodex context compress PATH --dry-run` first; compression is local and deterministic, skips backups, only edits Markdown/text prose files, and writes an `.original.md` backup before replacement.

Use `prodex gateway` when you need an OpenAI-compatible service endpoint instead of launching Codex:

```bash
PRODEX_GATEWAY_TOKEN=change-me GEMINI_API_KEY=... prodex gateway --provider gemini
auth_header="Authorization: Bearer $PRODEX_GATEWAY_TOKEN"
curl http://127.0.0.1:4000/v1/responses \
  -H "$auth_header" \
  -H "Content-Type: application/json" \
  -d '{"model":"prodex-fast","input":"hello"}'
```

The gateway serves `/v1/responses`, `/v1/chat/completions`, `/v1/embeddings`, `/v1/images/*`, `/v1/audio/*`, `/v1/batches`, `/v1/rerank`, `/v1/a2a`, `/v1/messages`, and `/v1/models` where the selected upstream supports them. It emits `x-prodex-call-id`, writes `gateway_spend` events for request and response phases to runtime logs, can export those events to JSONL or HTTP using generic, OTel, Datadog, or Langfuse-shaped payloads, and supports catalog-backed route strategies (`fallback`, `round-robin`, `least-busy`, `lowest-cost`, `lowest-latency`, `rpm`, `tpm`, `first`), static virtual keys with persisted request/spend usage and model/budget/RPM/TPM limits, file, SQLite, Postgres, or Redis-backed gateway admin/usage/ledger/SCIM state, plus keyword/model, Presidio, and external webhook guardrails. Admin-token, trusted-proxy SSO, or OIDC/JWT bearer requests can inspect usage, create generated-token keys, rotate/disable/update/delete admin-managed keys, provision SSO users through SCIM-compatible `/v1/prodex/gateway/scim/v2/Users`, read virtual-key usage at `/v1/prodex/gateway/keys` and `/v1/prodex/gateway/usage`, read recent billing ledger records with response-status/output-token reconciliation at `/v1/prodex/gateway/ledger`, read aggregated billing totals at `/v1/prodex/gateway/ledger/summary`, export billing CSV from `/v1/prodex/gateway/ledger.csv` and `/v1/prodex/gateway/ledger/summary.csv`, scrape Prometheus text metrics at `/v1/prodex/gateway/metrics`, fetch the machine-readable gateway contract at `/v1/prodex/gateway/openapi.json`, and open the built-in gateway admin dashboard at `/v1/prodex/gateway/admin`; policy/env-backed keys remain read-only, admin-managed key and SCIM user mutations are recorded in `prodex audit`, and additional admin-plane tokens can be `admin` or read-only `viewer` with optional virtual-key prefix and tenant scopes.

JavaScript clients can use `@christiandoxa/prodex-gateway-sdk` for gateway Responses, key, usage, billing ledger, metrics, and OpenAPI calls.

For the repository-provided Docker Compose scaffold, see [docs/deployment.md](./docs/deployment.md).

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

[gateway]
listen_addr = "127.0.0.1:4000"
provider = "gemini"
require_auth = true

[gateway.adaptive_routing]
enabled = true
shadow_mode = true
window_size = 128
min_samples = 8

[gateway.state]
backend = "sqlite"
sqlite_path = "gateway-state.sqlite"

[[gateway.admin_tokens]]
name = "auditor"
token_env = "PRODEX_GATEWAY_AUDITOR_TOKEN"
role = "viewer"
allowed_key_prefixes = ["team-a-"]
tenant_id = "tenant-a"

[[gateway.route_aliases]]
alias = "prodex-fast"
models = ["gemini-3-flash", "gemini-2.5-flash"]
strategy = "fallback"

[[gateway.virtual_keys]]
name = "team-a"
token_env = "PRODEX_GATEWAY_TEAM_A_TOKEN"
tenant_id = "tenant-a"
allowed_models = ["prodex-fast"]
request_budget = 1000
rpm_limit = 60
tpm_limit = 100000

[gateway.observability]
sinks = ["runtime-log", "jsonl"]
jsonl_path = "gateway-spend.jsonl"

[gateway.guardrails]
blocked_keywords = ["secret project"]
blocked_output_keywords = ["do not reveal"]
allowed_models = ["prodex-fast"]
webhook_url = "https://guardrails.example/check"
webhook_phases = ["pre", "post"]
```

Environment variables still override `policy.toml`.
Use `prodex info` to inspect the resulting effective runtime tuning values.
See [docs/runtime-policy.md](./docs/runtime-policy.md) for all `runtime`, `gateway`, and `runtime_proxy` keys, env overrides, defaults, and meanings.
Gateway adaptive routing is a shadow-mode foundation: it can score owner-attributed feedback and report recommendations, but deterministic routing and continuation affinity still decide live traffic.
Prodex's own keyring secret backend is not implemented yet. Selecting `PRODEX_SECRET_BACKEND=keyring` or `[secrets].backend = "keyring"` makes doctor/info report an invalid backend instead of silently claiming OS keyring storage is active.

## Admin And Observability Notes

Prodex is still a local-first tool, even with the current admin and observability hardening work.

Current local hardening includes:

- a secret-management abstraction for `auth.json` and export bundles, plus global secret-backend selection through policy or environment
- a stable broker metrics JSON endpoint at `/__prodex/runtime/metrics`
- a Prometheus broker metrics endpoint at `/__prodex/runtime/metrics/prometheus`
- `prodex info` and `prodex doctor --runtime --json` surfacing live metrics targets and the selected secret backend
- `prodex doctor --bundle PATH --redacted` for shareable local diagnostics without stored auth tokens
- local structured audit logging for profile, rotation, and admin events, separate from runtime session output and discoverable through `prodex info` or `prodex doctor --runtime --json`
- `prodex audit` for browsing the local append-only audit log without touching runtime proxy behavior
- Codex child-process env sanitization for dynamic-loader injection variables such as `LD_PRELOAD`, `LD_AUDIT`, `LD_LIBRARY_PATH`, and `DYLD_*`; set `PRODEX_ALLOW_UNSAFE_CHILD_ENV=1` only for deliberate local debugging

Known gaps today:

- local `auth.json` remains the compatibility source of truth for current Codex flows even when a non-file backend is selected
- no keychain, Vault, or KMS-backed secret storage implementation yet
- audit logs follow the resolved runtime log directory by default, or `PRODEX_AUDIT_LOG_DIR` when set
- gateway admin RBAC currently supports admin/viewer bearer tokens, trusted reverse-proxy SSO headers, native OIDC/JWT bearer verification with discovery, SCIM-compatible user provisioning, tenant-scoped admin boundaries, and a built-in dashboard, but it is still not a hosted central SaaS control plane
- runtime observability has local logs, `doctor --runtime --json`, gateway JSONL/HTTP event export, virtual-key usage reads, and Prometheus text metrics; it is still not a centralized admin plane
- Docker Compose plus file, SQLite, Postgres, and Redis gateway state backends exist for admin keys, usage counters, and billing ledger state
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
- `selection_plan`
- `precommit_budget_exhausted`
- `local_rewrite_provider_model_fallback`
- `local_rewrite_gemini_quota_rotate`
- `local_rewrite_gemini_invalid_stream_retry`
- `local_rewrite_gemini_live_error`
- `first_upstream_chunk`
- `first_local_chunk`
- `stream_read_error`

### Presidio redaction

When you enable Presidio with `prodex presidio enable` and use Super mode (e.g., `prodex s`), Prodex starts a dedicated runtime proxy that redacts sensitive information when the configured Presidio services are healthy; service failures follow `fail_mode`. Prodex now supports multi-language Presidio redaction.

The runtime uses `presidio.toml` endpoints and language configuration when available, falling back to `http://localhost:5002` and `http://localhost:5001` for Analyzer/Anonymizer URLs, and English (`en`) for language if not specified. It honors `fail_mode = "open"` or `"closed"`.

Example `presidio.toml` for multi-language (English and Indonesian) auto-detection:
```toml
enabled = true
analyzer_url = "http://localhost:5002"
anonymizer_url = "http://localhost:5001"
language_mode = "auto"
languages = ["en", "id"]
fail_mode = "open"
```

The default Presidio Docker images typically support English (`en`). For Indonesian (`id`), use a custom Presidio Analyzer with Indonesian NLP config plus recognizers; Prodex can route `en,id`, but detection quality comes from the Analyzer container.
