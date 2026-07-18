# prodex

`prodex` is a multi-account, multi-provider Codex wrapper with quota-aware profile routing.

Use multiple Codex accounts and supported provider backends from one command line. OpenAI/Codex profiles get quota-aware routing and can auto-rotate when multiple eligible profiles exist; provider adapters let `prodex s` launch the Codex front end against Gemini, Anthropic, Copilot, Kiro, DeepSeek, and local OpenAI-compatible servers.

![Prodex overview](https://github.com/christiandoxa/prodex/releases/download/assets/prodex-overview.png)

## Contents

- [Why prodex](#why-prodex)
- [Requirements](#requirements)
- [Supported providers](#supported-providers)
- [Installation](#installation)
- [Optional tools](#optional-tools)
- [Quick start](#quick-start)
- [Daily command: `prodex s`](#daily-command-prodex-s)
- [Commands](#commands)
- [Modes](#modes)
- [Harness modes](#harness-modes)
- [Profiles](#profiles)
- [Local model support](#local-model-support)
- [Utilities and diagnostics](#utilities-and-diagnostics)
- [Advanced behavior](#advanced-behavior)
- [Documentation](#documentation)
- [Support](#support)

## Why prodex

Use `prodex` if you want to:

- use multiple Codex accounts from one CLI
- rotate to another eligible account when quota runs out
- launch Codex/Super against non-OpenAI providers without changing front ends
- keep profile `auth.json` account credentials separated
- keep sessions attached to the profile that created them
- run Codex, Caveman mode, Super mode, and Claude Code through the same wrapper

If you only use one Codex account and do not need quota rotation, you probably do not need `prodex`.

## Requirements

For OpenAI/Codex quota-aware routing, you need at least one logged-in Prodex profile. Local `--url` launches and some provider API-key launches can run without a persisted profile.

<details>
<summary>Tool requirements</summary>

| Tool | Used by |
|---|---|
| Codex CLI | `prodex`, `prodex run`, `prodex caveman`, `prodex playwright`, `prodex super` |
| Claude Code | `prodex claude` |
| RTK | `rtk` variants and `prodex s` / `prodex super` |
| Node.js 18+ with `npx` | `prodex playwright` and Playwright MCP in `prodex s` / `prodex super` |

</details>

## Supported providers

Prodex supports two provider paths:

- **Profile-backed routing**: persisted profiles that Prodex can select, rotate, and inspect where provider APIs allow it.
- **Runtime provider launch**: `prodex s gemini`, `prodex s deepseek`, or `prodex s --provider ...` starts Codex with a temporary provider bridge for that session.

<details>
<summary>Supported provider matrix</summary>

| Provider | Launch path | Auth path | Quota view | Notes |
|---|---:|---|---:|---|
| OpenAI / Codex | `prodex`, `prodex run`, `prodex s` | ChatGPT OAuth, device code, or OpenAI/API-compatible key via `prodex login` | Yes | Quota preflight, plus profile auto-rotation when multiple eligible profiles exist. |
| Google Gemini | `prodex s gemini` or `prodex s gemini --cli gemini` | Google OAuth via `prodex login --with-google`, or `GEMINI_API_KEY(S)` / `GOOGLE_API_KEY(S)` / `--api-key` for the Codex front end | OAuth profiles | Native Gemini CLI uses the Prodex Code Assist OAuth proxy; API-key mode uses the Codex front end and Google's OpenAI-compatible Chat Completions endpoint. |
| Google Antigravity CLI | `prodex s gemini --cli agy` | Antigravity keyring / Google Sign-In via `prodex login --with-antigravity` or `agy auth login` | CLI quota snapshot | Native CLI path; no Prodex account auto-rotation or Presidio proxying. |
| Anthropic Claude | `prodex s --provider anthropic` | Claude Code OAuth via `prodex login --with-claude` / `prodex profile import claude`, or `ANTHROPIC_API_KEY(S)` / `--api-key` | OAuth profiles | Shows Claude OAuth readiness; add `ANTHROPIC_ADMIN_KEY` to include Anthropic Admin rate-limit groups. |
| GitHub Copilot | `prodex s --provider copilot` or `prodex s --provider copilot --cli copilot` | Imported Copilot CLI profile via `prodex profile import copilot`, or `GITHUB_COPILOT_API_KEY(S)` / `--api-key` | Imported profiles | Codex and native Copilot CLI front ends use the Prodex Responses adapter; fresh requests can rotate before commit and continuations stay bound to the owning profile. |
| Kiro CLI | `prodex s --provider kiro` or `prodex super --cli kiro` | Imported Kiro CLI profile via `prodex profile import kiro` | Imported profiles | Codex uses Prodex's Kiro ACP adapter; native Kiro uses an authenticated loopback CONNECT tunnel while its proprietary TLS payload remains opaque. |
| DeepSeek | `prodex s deepseek` | `DEEPSEEK_API_KEY(S)` / `--api-key` | API-key balance | `prodex quota --all --provider deepseek` reads DeepSeek `/user/balance`. |
| Local OpenAI-compatible | `prodex super --url http://127.0.0.1:8131` | Local server auth/config | Health snapshot | `prodex quota --all --provider local --base-url ...` checks the local `/models` endpoint. |
| Bedrock / custom Codex `model_provider` | `prodex run` / `prodex caveman` direct pass-through | Codex-owned config | Config snapshot | Prodex reports configured provider metadata; provider-side quota stays owned by Codex/upstream. |

</details>

`prodex gateway` exposes the provider bridge as a standalone OpenAI-compatible service for non-Codex clients:

<details>
<summary>Gateway quickstart</summary>

```bash
PRODEX_GATEWAY_TOKEN=change-me GEMINI_API_KEY=... prodex gateway --provider gemini
auth_header="Authorization: Bearer $PRODEX_GATEWAY_TOKEN"
curl http://127.0.0.1:4000/v1/responses \
  -H "$auth_header" \
  -H "Content-Type: application/json" \
  -d '{"model":"prodex-fast","input":"hello"}'
```

</details>

<details>
<summary>Gateway capabilities (advanced)</summary>

The gateway serves `/v1/responses`, `/v1/chat/completions`, `/v1/embeddings`, `/v1/images/*`, `/v1/audio/*`, `/v1/batches`, `/v1/rerank`, `/v1/a2a`, `/v1/messages`, and `/v1/models` where the selected upstream supports them. It adds `x-prodex-call-id` to responses, writes local request detail plus `gateway_spend` events for both `request` and `response` phases to runtime logs, can export those events to JSONL or HTTP using generic, OTel, Datadog, or Langfuse-shaped payloads, supports catalog-backed policy routing strategies (`fallback`, `round-robin`, `least-busy`, `lowest-cost`, `lowest-latency`, `rpm`, `tpm`, `first`) for model aliases/fallback chains, can enforce static virtual keys with persisted request/spend usage plus model/budget/RPM/TPM limits, supports file, SQLite, Postgres, or Redis-backed gateway admin/usage/ledger/SCIM state, and can apply keyword/model, local PII redaction, Presidio, and external webhook guardrails before calls and on outputs. Admin-token, trusted-proxy SSO, or OIDC/JWT bearer requests can list usage, create generated-token keys, rotate/disable/update/delete admin-managed keys, provision SSO users through SCIM-compatible `/v1/prodex/gateway/scim/v2/Users`, inspect usage at `/v1/prodex/gateway/keys` and `/v1/prodex/gateway/usage`, read recent billing ledger records with response-status/output-token reconciliation at `/v1/prodex/gateway/ledger`, read aggregated billing totals at `/v1/prodex/gateway/ledger/summary`, export billing CSV from `/v1/prodex/gateway/ledger.csv` and `/v1/prodex/gateway/ledger/summary.csv`, scrape Prometheus text metrics at `/v1/prodex/gateway/metrics`, inspect provider adapter contracts at `/v1/prodex/gateway/providers` or offline with `prodex gateway providers --json`, inspect active observability and guardrail configuration at `/v1/prodex/gateway/observability` and `/v1/prodex/gateway/guardrails`, fetch the machine-readable gateway contract at `/v1/prodex/gateway/openapi.json`, and open the built-in gateway admin dashboard at `/v1/prodex/gateway/admin`; policy/env-backed keys remain read-only, SCIM users can carry tenant/team/project/user/budget scopes for SSO/OIDC fallback, admin-managed key and SCIM user mutations are recorded in `prodex audit`, and additional admin-plane tokens can be `admin` or read-only `viewer` with optional virtual-key prefix plus tenant/team/project/user/budget scopes. Configure defaults under `[gateway]` in `policy.toml`; validate provider catalog edits with `npm run catalog:providers`. The generated provider matrix lives in [docs/provider-capabilities.md](./docs/provider-capabilities.md).

The gateway can enforce optional model-aware request constraints under `[gateway.request_constraints]`; compatibility defaults leave enforcement disabled and oversized output requests unchanged. Admin/viewer principals can use the dashboard Route Workbench or `POST /v1/prodex/gateway/routes/explain` to inspect the same bounded planner trace without sending upstream traffic or mutating quota, billing, affinity, circuit, admission, or persisted runtime state. Explain payloads and prompt content are not logged or stored.

Enterprise OTLP export endpoints from `OTEL_EXPORTER_OTLP_LOGS_ENDPOINT` or `OTEL_EXPORTER_OTLP_ENDPOINT` must be absolute `http://` or `https://` URLs without whitespace, userinfo, query strings, or fragments; put collector credentials in `OTEL_EXPORTER_OTLP_HEADERS`. `prodex-control-plane plan-http-control-plane` request files must use the top-level `principal` field and non-credential HTTP headers, because `Authorization` headers are rejected.

`[gateway.adaptive_routing]` is opt-in. With `enabled = true`, Prodex keeps bounded per-model outcome/latency windows and can reorder only fresh pre-commit alias fallbacks; `shadow_mode = true` records the recommendation without changing selection. Hard continuation affinity, quota filtering, and the no-mid-stream-rotation boundary remain authoritative. Optional `exploration_rate` is deterministic per request and bounded from `0.0` to `1.0`.

JavaScript clients can use `@christiandoxa/prodex-gateway-sdk` for `/v1/responses` plus gateway key, usage, billing ledger, metrics, and OpenAPI admin calls.

</details>

<details>
<summary>Provider behavior details (advanced)</summary>

The auto-rotate proxy is intentionally conservative. It rotates only before a request or stream is committed, preserves `previous_response_id`, turn-state, and session affinity, and does not rotate mid-stream. Prodex does not auto-redeem reset credits by default. If you launch an OpenAI/Codex runtime path with `--auto-redeem`, Prodex may redeem one earned reset credit only when the weekly usage-limit window is exhausted, no other profile in the quota pool still has weekly quota remaining, and the weekly reset is not already imminent, then retries the same profile before rotating. It still does not redeem for merely critical/thin windows or 5h-only exhaustion. You can also run `prodex redeem <profile>` to send one explicit reset-credit consume request for a named OpenAI/Codex profile; the upstream backend decides whether that manual request applies, reports nothing-to-reset, or reports no-credit/already-redeemed. OpenAI/Codex remains the default quota-aware pool. Gemini OAuth, Antigravity CLI, imported Copilot profiles, Anthropic OAuth profiles, DeepSeek API keys, local OpenAI-compatible URLs, and Bedrock/custom Codex providers now have `prodex quota` views. Anthropic, DeepSeek, API-key Gemini, API-key Copilot, Antigravity CLI, local URLs, and Bedrock/custom Codex providers still skip OpenAI quota preflight.

Runtime proxy design contract:

- Prodex stays a scoped Codex gateway, not a general-purpose LLM SDK.
- Profile selection must be visible through policy, `prodex info`, `prodex doctor`, and runtime logs.
- Pre-commit retry and fallback paths must stay bounded per request.
- Runtime hot paths must avoid broad disk reads, quota probes, or blocking state saves.
- Quota, budget, transport, and local pressure signals must stay classified separately.
- Selection, admission, affinity, backoff, and first-chunk events must be structured in runtime logs.
- Upstream HTTP/WebSocket connection reuse should be preserved where it does not change Codex semantics.
- Secrets remain profile-isolated, redacted in diagnostics, and covered by audit events for Prodex-owned mutations.

</details>

## Installation

Install the standalone macOS or Linux binary from the latest GitHub Release:

```bash
curl -fsSL https://github.com/christiandoxa/prodex/releases/latest/download/install.sh | sh
```

On Windows, run this from PowerShell or Command Prompt:

```powershell
powershell -ExecutionPolicy ByPass -c "irm https://github.com/christiandoxa/prodex/releases/latest/download/install.ps1 | iex"
```

<details>
<summary>Installer verification, alternate source, and legacy migration</summary>

The installers download the matching release asset and verify it against `SHA256SUMS`. The macOS/Linux target is `~/.local/bin`; Windows uses `%LOCALAPPDATA%\Programs\Prodex\bin`. Their auditable sources are also available directly from the repository:

```bash
curl -fsSL https://raw.githubusercontent.com/christiandoxa/prodex/main/install.sh | sh
```

```powershell
powershell -ExecutionPolicy ByPass -c "irm https://raw.githubusercontent.com/christiandoxa/prodex/main/install.ps1 | iex"
```

Set `PRODEX_INSTALL_DIR` to choose another binary directory. Standalone installs use the `codex` command on `PATH`; install Codex first if it is not already available.

npm and Cargo installations are no longer supported. Existing copies from either legacy channel should run `prodex update` once to migrate to the standalone installer. Contributors should use normal workspace development commands such as `cargo build` instead of treating a source build as a supported installation channel.

</details>

## Optional tools

Prodex Super keeps a deliberately small optional stack:

- [RTK](https://github.com/rtk-ai/rtk) for noisy shell output.
- [Codebase Memory MCP](https://github.com/DeusData/codebase-memory-mcp) for structural code navigation.
- [Playwright MCP](https://github.com/microsoft/playwright-mcp) for browser inspection and automation.
- [Ponytail](https://github.com/DietrichGebert/ponytail) for minimal-implementation guidance.
- [Presidio](https://github.com/data-privacy-stack/presidio) for opt-in PII redaction.

Caveman and Smart Context are built into Prodex. Every default Codex-based `prodex s` or `prodex playwright` launch adds a pinned Playwright MCP server to its temporary overlay when Node.js 18+ and `npx` are available. Prodex runs without every external tool above; missing tools are skipped instead of blocking launch.

<details>
<summary>Install and verify the Super tools</summary>

RTK:

```bash
brew install rtk-ai/tap/rtk
# or
cargo install --git https://github.com/rtk-ai/rtk rtk

rtk --version
rtk gain
```

Codebase Memory MCP:

```bash
curl -fsSL https://raw.githubusercontent.com/DeusData/codebase-memory-mcp/main/install.sh | bash -s -- --skip-config
codebase-memory-mcp --help
```

Playwright MCP (Prodex currently pins `@playwright/mcp@0.0.78`):

```bash
node --version
npx --version
npx -y @playwright/mcp@0.0.78 --version
prodex playwright
```

Playwright starts through `npx` in headless, isolated mode, so concurrent Prodex terminals do not share browser login state. It requires a browser usable by Playwright and prompts before tools marked as writes. Playwright MCP is not a security boundary.

Prodex preserves inherited `[mcp_servers.playwright]` entries. Add a custom entry to the base profile's `config.toml` to change flags, use a persistent/headed browser, or set `enabled = false`; the temporary Super overlay will not replace it.

Ponytail:

```bash
git clone https://github.com/DietrichGebert/ponytail.git ~/.local/share/prodex-optimizers/ponytail
prodex ponytail
```

Prodex installs Ponytail only into the temporary overlay for that session. The base Codex profile remains unchanged.

Presidio English services:

```bash
docker run -d --name presidio-analyzer -p 5002:3000 mcr.microsoft.com/presidio-analyzer:latest
docker run -d --name presidio-anonymizer -p 5001:3000 mcr.microsoft.com/presidio-anonymizer:latest
prodex presidio enable --language-mode fixed --languages en
prodex presidio doctor --json
```

The standard Analyzer image is English-only. Indonesian detection requires an Analyzer configured with Indonesian NLP models and recognizers before enabling `--language-mode auto --languages en,id`.

Verify the complete stack:

```bash
prodex s doctor --presidio --strict
prodex s
```

</details>

## Quick start

<details>
<summary>Import your current Codex login</summary>

If your current Codex home is already logged in:

```bash
prodex profile import-current main
```

</details>

<details>
<summary>Create profiles from scratch</summary>

```bash
prodex login
prodex profile add second
prodex login --profile second
prodex login --with-google
prodex login --with-claude
prodex login --with-antigravity
```

Interactive `prodex login` now asks for the login method before starting a browser. Choose ChatGPT browser login, device-code login, API-key login, Google sign-in for Gemini, Claude sign-in through Claude Code OAuth, or Antigravity CLI sign-in through `agy auth login`. Antigravity login is global to the `agy` CLI and does not create a Prodex profile. For API-key profiles, you can also set an OpenAI-compatible backend URL:

```bash
printf '%s\n' "$OPENAI_API_KEY" | prodex login --with-api-key --base-url http://localhost:11434/v1
```

Migration note: endpoint and base-URL inputs are credential-free. Runtime and quota `--base-url` values, Super `--url`, `CODEX_CHATGPT_BASE_URL`, stored OpenAI-compatible profile URLs, Presidio Analyzer/Anonymizer URLs, and gateway webhook/HTTP telemetry endpoints must be absolute `http` or `https` URLs with a host and no userinfo, password, query, or fragment. Move credentials to the existing API-key, auth-token, bearer-token environment, or secret-file inputs. Legacy embedded-credential URLs now fail closed before a request, log, broker registry, or child launch plan is created instead of being normalized or partially stripped.

</details>

<details>
<summary>Check profiles and quota</summary>

```bash
prodex profile list
prodex quota --all
prodex session list
```

</details>

<details>
<summary>Start Codex through Prodex</summary>

```bash
prodex
```

Or run a one-off prompt:

```bash
prodex exec "review this repo"
```

</details>

<details>
<summary>Import a Claude Code account</summary>

```bash
prodex profile import claude
prodex profile import claude --name claude-main --activate
```

This imports the current Claude Code OAuth credentials from `CLAUDE_CONFIG_DIR` or `~/.claude` into a Prodex-managed Anthropic profile. You can also use `prodex login --with-claude` to sign in through Claude Code directly.

</details>

<details>
<summary>Import a Copilot CLI account</summary>

```bash
prodex profile import copilot
prodex profile import copilot --name copilot-main --activate
```

When you import a Copilot profile, Prodex does not move the Copilot token into Prodex-managed storage. It only records the provider identity and API endpoint in its own metadata.

</details>

<details>
<summary>Import a Kiro CLI account</summary>

```bash
prodex profile import kiro
prodex profile import kiro --name kiro-main --activate
```

This reads the installed Kiro CLI state from the local auth database, snapshots the current Kiro auth payload into the managed Prodex profile, and refreshes a Kiro model catalog snapshot for later `--provider kiro` or `--cli kiro` launches. Override the detected CLI binary with `PRODEX_KIRO_BIN` when needed.

</details>

## Daily command: `prodex s`

`prodex s` is the daily alias for `prodex super`. It enables:

- Caveman and Ponytail.
- RTK shell-output guidance.
- Codebase Memory MCP when installed.
- Playwright MCP when Node.js 18+ and `npx` are available.
- Smart Context Autopilot.
- launch-time full access.
- optional Presidio redaction.

```bash
prodex s
prodex s exec "review this repo"
prodex s doctor --strict
prodex s doctor --presidio --strict
prodex s expose
```

<details>
<summary>Super launch details</summary>

The effective launch is:

```bash
prodex caveman rtk ponytail --full-access
```

Answer `y` at the Presidio prompt, or pass `--presidio`, to add runtime PII redaction. Use `--no-presidio` for non-interactive launches. Full access maps to Codex's sandbox bypass and trusts only the launch directory for that session.

`prodex s expose` starts a loopback-only browser terminal with a one-time session URL. Add `--tunnel` to explicitly publish the remote shell through a Cloudflare quick tunnel; the local listener remains bound to loopback.

Smart Context preserves continuation metadata and critical signals while applying deterministic, validated context rewriting. See [docs/smart-context.md](docs/smart-context.md) for its safety model and rollout controls.

Managed optimizer roots are checked in this order: `PRODEX_OPTIMIZERS_HOME`, `$XDG_DATA_HOME/prodex-optimizers`, then `~/.local/share/prodex-optimizers`.

</details>

## Commands

<details>
<summary>Most used commands</summary>

```bash
prodex
prodex s
prodex s expose
prodex exec "review this repo"
prodex quota --all
prodex profile list
prodex session list
```

</details>

<details>
<summary>Run Codex</summary>

```bash
prodex
prodex run
prodex run --profile main
prodex run --dry-run
prodex exec "review this repo"
prodex delete 019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9
```

Codex-owned TUI commands such as `/usage`, `/goal`, `/import`, and `/delete` stay upstream Codex behavior. Prodex preserves their request metadata through the proxy and does not add a competing command surface. If an active goal reaches Codex's `usage_limited` state while the TUI remains open, Prodex waits for another quota-ready OpenAI profile, gracefully relaunches the same session, releases its old affinity, and invokes `/goal resume`; `--no-auto-rotate` disables this recovery. The CLI form `prodex delete <session>` passes through to Codex and, after a successful delete, prunes matching Prodex session affinity metadata.

</details>

<details>
<summary>Run Super mode</summary>

```bash
prodex s
prodex s exec "review this repo"
prodex super
prodex super --profile main
prodex super --dry-run
```

</details>

<details>
<summary>Check quota</summary>

```bash
prodex quota --all
prodex quota --all --once
prodex quota --all --auth no-auth --once
prodex quota --all --detail --provider openai
prodex quota --all --provider deepseek --once
prodex quota --all --provider local --base-url http://127.0.0.1:8131/v1 --once
prodex redeem main
prodex dashboard
prodex status
```

`prodex status` opens a btop-inspired live terminal dashboard combining the active/runtime profile, 5-hour and weekly quota/reset/runway, historical token usage and cache efficiency, and aggregate Prodex process CPU, resident memory, disk I/O, and network socket queues. Press `r` to refresh immediately and `q` or `Esc` to exit. `prodex status --once` emits one snapshot for scripts. Resource counters use Linux `/proc`; non-Linux systems show those fields as unavailable while quota and token panels continue working.

The live `prodex quota --all --detail` view accepts `s` to cycle sort modes and `f` to cycle the provider filter through `all`, `openai`, `gemini`, `anthropic`, `copilot`, `kiro`, `deepseek`, and `local`. Add `--provider openai`, `--provider gemini`, `--provider anthropic`, `--provider copilot`, `--provider kiro`, `--provider deepseek`, or `--provider local` to start locked to a single provider.

For OpenAI/Codex profiles, quota views also show earned rate-limit reset credits when the upstream usage API reports them. Use `prodex redeem <profile>` when you explicitly want to redeem one reset credit on a named profile, even if the 5h and weekly quota windows still have remaining quota. If either quota window resets within 1 hour, Prodex asks before consuming the credit; pass `--yes` to skip that prompt. Add `--auto-redeem` to a runtime launch when you want Prodex to consider a guarded automatic redeem after every OpenAI/Codex profile is weekly-exhausted.

`prodex dashboard` serves a local browser control plane at `http://127.0.0.1:8765` by default. Open it first when you are unsure what to do next:

1. **Open dashboard:** `prodex dashboard`
2. **Add/import/login provider:** choose OpenAI, Gemini, Anthropic, Copilot, DeepSeek, Kiro, or local OpenAI-compatible setup and copy the generated command.
3. **Pick provider/model:** use the Models section to see recommended models, context windows, capabilities, and launch commands.
4. **Run:** `prodex s` for the active OpenAI/Codex pool, or `prodex s --provider ...` / `prodex s gemini` / `prodex super --url ...` for provider paths.
5. **Check quota/logs:** use Usage and Runtime/Gateway sections, or run `prodex quota --all --once` and `prodex doctor --runtime`.

The dashboard shows profile/account settings, active profile controls, provider presets from Prodex's provider catalog, model catalog metadata, runtime/gateway pointers, and live usage from the same quota collectors used by `prodex quota`. Provider setup is conservative: the dashboard generates safe commands instead of storing provider secrets. Use `prodex dashboard --port 0` for an OS-selected free port, or pass `--base-url` for quota checks against a custom Codex-compatible backend. The dashboard has no password auth; keep it on localhost unless the network is trusted.

</details>

<details>
<summary>Sessions</summary>

```bash
prodex session list
prodex session current
prodex session current --parent-only
```

</details>

<details>
<summary>Update Prodex</summary>

```bash
prodex update --help
prodex update
```

`prodex update` downloads the latest checksum-verified GitHub Release binary on macOS, Linux, and Windows. Existing npm or legacy Cargo installations migrate to the standalone path. Update notices state that both legacy installation channels are unsupported and direct them to this command.

</details>

<details>
<summary>More Codex command examples</summary>

```bash
prodex run 019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9
printf 'context from stdin' | prodex run exec "summarize this"
```

Unknown top-level Codex subcommands remain managed Codex launches.

For example:

```bash
prodex remote-control
```

is equivalent to:

```bash
prodex run remote-control
```

unless Prodex explicitly owns that command.

</details>

<details>
<summary>Codex runtime feature overrides and app-server compatibility (advanced)</summary>

Prodex keeps recent Codex runtime switches as Codex-owned behavior by rendering launch flags into `codex -c ...` overrides on `prodex run`, `prodex caveman`, and `prodex super`:

```bash
prodex run --web-search indexed
prodex run --web-search cached
prodex run --web-search live
prodex run --web-search disabled

prodex run --rollout-budget-tokens 100000
prodex run --rollout-budget-tokens 100000 --rollout-budget-reminders 75000,50000,25000

prodex run --current-time-reminder
prodex run --current-time-reminder --current-time-reminder-interval 2
prodex run --respect-system-proxy
```

`--web-search` maps to Codex's top-level `web_search = "disabled" | "cached" | "indexed" | "live"` setting. In Super provider mode, an explicit `--web-search` is appended after the provider default, so it overrides the default bridge choice.

`--rollout-budget-tokens` enables Codex's `[features.rollout_budget]` config. If no reminder thresholds are supplied, Prodex provides valid 75%, 50%, and 25% remaining-token thresholds for the selected limit. Use `--rollout-budget-sampling-weight` and `--rollout-budget-prefill-weight` only when you need Codex's weighted accounting knobs.

`--current-time-reminder` enables Codex's `[features.current_time_reminder]` config. The default system clock source is owned by Codex. `--current-time-clock-source external` is intended for Codex app-server clients that implement the upstream `currentTime/read` request.

`--respect-system-proxy` enables Codex's `[features.respect_system_proxy]` config when the bundled/upstream Codex supports it. Codex 0.143.0 extends this path to auth and Responses API traffic on Windows and macOS so PAC, WPAD, static system proxy, and bypass decisions can be honored. `--no-respect-system-proxy` renders an explicit false override for sessions that need the upstream default direct/env-proxy behavior.

Codex `multiAgentMode` is an app-server/thread setting, not a normal TUI `config.toml` launch override. Prodex therefore does not invent a competing CLI config flag. Launch `prodex app-server` or `prodex run app-server` and pass upstream `multiAgentMode` values (`none`, `explicitRequestOnly`, or `proactive`) through the Codex app-server API.

`prodex mcp-server`, `prodex app-server`, and `prodex exec-server` are direct Codex command-server passthroughs by default. Prodex selects the profile `CODEX_HOME` and preserves upstream protocol arguments, but it does not wrap stdio, inject the runtime proxy, rotate accounts inside the protocol, or apply gateway guardrails to those command-server streams. The JSON-RPC-aware app-server broker is available only behind explicit opt-in; default passthrough remains the compatibility path.

`prodex app-server-broker --json` exposes the live-validated broker contract. It recognizes JSON-RPC lifecycle methods such as `initialize`, `thread/start`, `thread/resume`, `thread/fork`, `turn/start`, and `turn/interrupt`, while still accepting compatibility aliases such as `notifications/initialized` and `turn/cancel`. The parser matches upstream wire behavior where `jsonrpc: "2.0"` may be omitted and advertises ordered continuation decisions as `fresh`, `continue-session`, `continue-thread`, and `continue-turn`.

The broker classifies newline-delimited JSON-RPC frames as `request`, `notification`, `response`, or `invalid`; bounds stdio reads and rejects lines over 1 MiB before JSON parsing; validates envelope shape, IDs, params, method names, response/error payloads, and lifecycle order; derives session/thread/turn/item metadata plus ordered affinity keys; and exposes invalid-reason counters. Secret-looking JSON-RPC string fields are redacted from diagnostics and logs. `--experimental-stdio` runs a diagnostic preview, `--experimental-stdio-passthrough-preview` mirrors input with diagnostics on stderr, `--experimental-stdio-validate` fails on invalid input, and `--experimental-stdio-validate-passthrough` forwards only valid frames. `--experimental-stdio-live [--profile NAME]` launches the selected profile's real `codex app-server`, validates both client-to-server and server-to-client streams against one shared lifecycle session, forwards only validated frames, and terminates the child on protocol or transport failure. Default Codex app-server passthrough remains unchanged; the broker does not invent provider routing. Each broker session appends one counts-only local `prodex audit` summary. Schema/replay drift fixtures live under `crates/prodex-app/tests/fixtures/compat_replay/`.

Codex plugin catalog commands are managed passthrough by default:

```bash
prodex plugin list
prodex plugin marketplace
```

</details>

## Modes

| Mode | Command | Description |
|---|---|---|
| Normal Codex | `prodex` or `prodex run` | Managed Codex launch with profile selection and quota routing. |
| Caveman | `prodex caveman` | Runs Codex with Caveman mode enabled. |
| Super | `prodex s` or `prodex super` | Daily mode with Caveman, RTK guidance, full access, and deterministic/local token optimizations. |
| Claude Code | `prodex claude` | Runs Claude Code through Prodex-managed state. |

<details>
<summary>Normal Codex — managed Codex launch</summary>

```bash
prodex
prodex run
prodex run --profile main
prodex exec "review this repo"
```

</details>

<details>
<summary>Caveman mode — runs Codex with Caveman enabled</summary>

```bash
prodex caveman
prodex rtk
prodex playwright
prodex ponytail
prodex caveman --dry-run
prodex s doctor
prodex s doctor --json --strict
prodex caveman --profile main
prodex caveman exec "review this repo in caveman mode"
prodex caveman 019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9
```

`prodex caveman` runs Codex with Caveman mode active in a temporary Prodex overlay `CODEX_HOME`, so the base profile home stays unchanged after the session ends.

Add `rtk`, `playwright`, `ponytail`, or `presidio` before Codex args to enable that session surface. The `prodex rtk`, `prodex playwright`, and `prodex ponytail` shortcuts map to `prodex caveman <prefix>`.

RTK is still an external binary. Install it separately if `rtk gain` is unavailable.

</details>

<details>
<summary>Super mode — daily Caveman + RTK + optimizer stack</summary>

```bash
prodex s
prodex s exec "review this repo"
ANTHROPIC_API_KEY=... prodex s --provider anthropic --model claude-sonnet-4-6
prodex profile import copilot
prodex s --provider copilot --model gpt-5.3-codex
DEEPSEEK_API_KEY=... prodex s deepseek --model deepseek-v4-pro
prodex s gemini
prodex super
prodex super --profile main
prodex super --dry-run
prodex super 019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9
```

`prodex s` is the short alias for `prodex super`.

This is my daily mode. It is the path I keep tuning for normal work: Caveman enabled, RTK guidance enabled, full access available, and context handling handled by the runtime proxy.

Super also enables Smart Context Autopilot in the runtime proxy.

<details>
<summary>Provider launch examples and bridge behavior (advanced)</summary>

Use `--provider anthropic` when you want the Codex/Super front end with Anthropic upstream:

```bash
prodex login --with-claude
prodex s --provider anthropic --model claude-sonnet-4-6
```

If `--api-key` is omitted, Prodex uses the Anthropic profile created by `prodex login --with-claude` or `prodex profile import claude`. API-key mode still reads `ANTHROPIC_API_KEY`; `ANTHROPIC_API_KEYS` may contain multiple comma-, semicolon-, or newline-separated keys for round-robin request rotation and pre-commit retry on auth/quota/rate/temporary failures. This path injects a temporary `prodex-anthropic` Codex provider, exposes a local `/v1/responses` adapter to Codex, forwards to Anthropic's OpenAI-compatible chat API, and keeps quota preflight disabled. `prodex quota --all --provider anthropic` shows OAuth readiness for Anthropic profiles; set `ANTHROPIC_ADMIN_KEY` when you want Anthropic Admin rate-limit groups included.

Provider-backed Super launches consume supported provider API-key environment variables in the local Prodex proxy and remove them from the spawned Codex environment. Child MCP servers or tools that previously relied on inheriting those variables must configure their own credential source explicitly.

Use `--provider copilot` when you want the Codex/Super front end with GitHub Copilot upstream:

```bash
prodex profile import copilot
prodex s --provider copilot --model gpt-5.3-codex
prodex s --provider copilot --cli copilot --model gpt-5.3-codex
```

Without `--api-key`, Prodex uses imported Copilot CLI profiles, resolves the stored Copilot OAuth token before launch, refreshes the Copilot model catalog, can rotate fresh native Responses requests across multiple eligible profiles, and binds streaming response IDs back to the owning profile for continuations. `GITHUB_COPILOT_API_KEY`, `GITHUB_COPILOT_API_KEYS`, or `--api-key` can be used when you already have a usable Copilot API bearer token; plural keys may be comma-, semicolon-, or newline-separated and can rotate before commit on auth/quota/rate/temporary failures.

Add `--cli copilot` to keep the native GitHub Copilot CLI front end while routing its OpenAI Responses traffic through the same local Prodex adapter. Prodex configures Copilot's documented custom-provider environment for OpenAI Responses over HTTP, sends only a synthetic local key to the child, and keeps imported account or API-key credentials inside the proxy. `PRODEX_COPILOT_BIN` overrides the `copilot` executable.

Use `--provider kiro` or `--cli kiro` when you want the Codex/Super front end or native Kiro CLI with imported Kiro credentials:

```bash
prodex profile import kiro
prodex s --provider kiro --model claude-sonnet-4.5
prodex super --cli kiro --profile kiro-main
```

`prodex profile import kiro` reads the installed Kiro CLI auth database (`~/.local/share/kiro-cli/data.sqlite3` or the Amazon Q compatibility location when present), snapshots the current credential payload into `kiro_auth.json`, and stores a model catalog snapshot for runtime routing. `--provider kiro` routes Codex through Prodex's local Kiro ACP adapter. `--cli kiro` launches the native Kiro CLI from the imported snapshot and forces its HTTP(S) transport through an authenticated loopback Prodex CONNECT tunnel; `--no-proxy` disables only an outer system proxy. Kiro's proprietary service payload remains end-to-end TLS encrypted, so native Kiro does not gain Smart Context, Presidio, response translation, or account rotation. Native Kiro rejects `--presidio` instead of silently ignoring it. Override binary discovery with `PRODEX_KIRO_BIN` when the installed launcher is not on `PATH`.

Use `--provider deepseek` when you want the Codex/Super front end with DeepSeek as the upstream model:

```bash
prodex s deepseek --model deepseek-v4-pro
```

If `--api-key` is omitted, Prodex reads `DEEPSEEK_API_KEY`; `DEEPSEEK_API_KEYS` may contain multiple comma-, semicolon-, or newline-separated keys for round-robin request rotation and pre-commit retry on auth/quota/rate/temporary failures. This path injects a temporary `prodex-deepseek` Codex provider, exposes a local `/v1/responses` adapter to Codex, forwards to DeepSeek's OpenAI-format chat API, and keeps quota preflight disabled. Prodex also injects a one-model Codex catalog for the selected DeepSeek model, so `/model` stays on that model and offers the DeepSeek-compatible `high`/`xhigh` effort choices. `prodex quota --all --provider deepseek` reads the same `DEEPSEEK_API_KEY(S)` environment and fetches DeepSeek `/user/balance`. Available Super optimizer tools remain local Prodex overlay additions around Codex. Remote compact is not implemented for this adapter yet, so the default DeepSeek context window is large and `--auto-compact-token-limit` defaults high.

The DeepSeek catalog includes `deepseek-v4-pro` and `deepseek-v4-flash`; `deepseek-chat` and `deepseek-reasoner` remain compatibility aliases for existing configs.

DeepSeek compatibility is translated, not native Responses. Prodex maps Codex text turns, function/MCP/local shell/apply-patch style tools, `tool_choice`, reasoning effort, JSON object mode, stop sequences, token limits (`max_output_tokens`, `max_tokens`, and `max_completion_tokens`), sampling, logprobs, streaming usage, and DeepSeek cache hit/miss usage into compatible shapes. Request `metadata`, `client_metadata`, `prompt_cache_key`, and `prompt_cache_retention` are preserved in local response metadata instead of being forwarded upstream. JSON schema requests are degraded to DeepSeek `json_object` mode and marked in response metadata because DeepSeek's OpenAI Chat route does not provide native JSON Schema enforcement. Web search is explicit and configurable: the DeepSeek catalog does not advertise native Codex web search, `[deepseek] web_search_mode = "auto"` fails clearly because DeepSeek's OpenAI Chat docs do not document native web-search request fields, `"off"` rejects web-search tools, and `"openai_chat"` explicitly chooses best-effort `web_search_options` forwarding with retry fallback. Gateway/profileless launches can use `PRODEX_DEEPSEEK_WEB_SEARCH_MODE`. The reserved `anthropic` and `function_proxy` modes fail clearly until the DeepSeek Anthropic adapter or a local search backend is implemented. DeepSeek function tools are bounded by the upstream 128-tool limit; Prodex must fail rather than silently truncate if that ceiling is reached, translated duplicate tool names are rejected instead of being dropped, and named `tool_choice` must target a translated function tool. Tool declarations must also be translatable: function/custom tools require names, namespace tools require a named namespace with named function entries, MCP toolsets that declare inventories require a server name plus allowed/enabled tools, and DeepSeek function names must use only letters, numbers, underscores, or dashes within the upstream 64-character limit. When DeepSeek thinking is enabled, Prodex omits explicit `tool_choice` for upstream compatibility and records the omitted value in DeepSeek response metadata. Reasoning content, refusal text, annotations, logprobs, and finish reasons are preserved in DeepSeek response metadata. Reasoning summaries are not advertised for DeepSeek, and `reasoning.summary` fails clearly. Image, document, audio, and video message content is rejected because this adapter targets DeepSeek's text/tool OpenAI Chat surface. Deprecated `frequency_penalty` and `presence_penalty` are not mapped because current DeepSeek docs mark them unsupported/ignored. `parallel_tool_calls=true` is compatible with DeepSeek's one-or-more tool-call behavior, but `parallel_tool_calls=false` is rejected because the OpenAI Chat route has no equivalent control to enforce it. Responses-only controls that DeepSeek cannot honor, including non-empty `include`, `store=false`, background responses, `truncation=auto`, per-message `cache_control`, `text.verbosity`, legacy `functions`/`function_call`, `logit_bias`, and `max_tool_calls`, fail clearly instead of being dropped.

DeepSeek troubleshooting: if JSON mode appears to stream whitespace or stalls, make sure the prompt explicitly asks for JSON; Prodex adds a minimal adapter instruction when JSON mode is requested, but model-facing task instructions still matter. A DeepSeek 400 around web search usually means the selected `openai_chat` web-search forwarding shape is not accepted by upstream; use `web_search_mode = "off"` or remove web-search tools unless you are deliberately testing that best-effort path. Strict tool failures usually mean a schema keyword outside DeepSeek's beta strict subset, a missing `required` property, or an object schema that cannot be made `additionalProperties = false`.

DeepSeek compatibility matrix:

| Capability | Status |
| --- | --- |
| Text chat, streaming text, usage | Translated through DeepSeek OpenAI Chat. |
| Reasoning effort and `reasoning_content` | Translated/preserved through DeepSeek thinking fields and response metadata. |
| JSON object output | Native DeepSeek `response_format = json_object`. |
| JSON Schema structured output | Degraded to `json_object` with DeepSeek degradation metadata. |
| Function tools, MCP tools, local shell, apply patch, `tool_search` | Translated to DeepSeek function tools and mapped back to Codex-compatible output items. |
| `tool_choice` with thinking | Omitted for upstream compatibility and recorded in DeepSeek response metadata. |
| Strict function tools | Beta only, opt in with `deepseek.strict_tools = true`. |
| Web search | Not advertised as native; `auto`/`off` fail clearly, `openai_chat` is explicit best-effort forwarding, `anthropic`/`function_proxy` are reserved until implemented. |
| Images, documents, audio, video, vision detail | Unsupported on this text/tool adapter; requests fail clearly. |
| Chat prefix completion, FIM `/completions` | DeepSeek beta features, not enabled on the current `/responses` adapter; completion-shaped requests fail fast. |
| Remote compact | Unsupported for DeepSeek today; use the large default context/auto-compact limit. |

Strict DeepSeek function calling is opt-in because DeepSeek documents it on the beta endpoint. Add this to the selected Codex profile config:

```toml
[deepseek]
strict_tools = true
beta_base_url = "https://api.deepseek.com/beta"
```

Gateway/profileless launches can use `PRODEX_DEEPSEEK_STRICT_TOOLS=1` and optional `PRODEX_DEEPSEEK_BETA_BASE_URL`. When enabled, Prodex routes rewritten DeepSeek `/responses` traffic through the beta base URL, sets every translated function tool to `strict: true`, forces strict object schemas to require all properties with `additionalProperties = false`, and rejects unsupported strict schema keywords or types clearly. DeepSeek beta chat prefix completion and FIM `/completions` are not enabled by this adapter yet; `prefix`, `prompt`, and `suffix` completion-style requests fail fast instead of being rewritten as chat.

Use `--provider gemini` when you want the Codex/Super front end with Gemini upstream:

```bash
prodex login --with-google
prodex s gemini
prodex s gemini --cli gemini
prodex s gemini --cli agy
GEMINI_API_KEY=... prodex s gemini --model gemini-2.5-pro
```

Without `--api-key`, Prodex uses the Google OAuth profile created by `prodex login --with-google` or the interactive Google sign-in choice, then routes through Google's Code Assist Gemini endpoint. Google login verifies Code Assist readiness before creating or updating the profile, and may open a second browser page if Google requires account verification. With `--api-key`, or `GEMINI_API_KEY(S)` / `GOOGLE_API_KEY(S)`, Prodex converts Codex Responses requests to Chat Completions and sends them through Google's documented `/v1beta/openai/chat/completions` endpoint with Bearer authentication. Streaming, function calls, continuations, and Gemini `reasoning_effort` values are converted back into Codex Responses semantics. Plural key env vars may be comma-, semicolon-, or newline-separated and can rotate before commit on auth/quota/rate/temporary failures. OAuth sessions keep fresh Gemini requests sticky to the previous successful profile by default for smoother Codex-style continuity; set `PRODEX_GEMINI_STICKY_FRESH_OAUTH=0` to restore pure fresh-request round robin. The default model is `auto`, matching Gemini CLI-style model routing through Gemini 3 and stable fallbacks; launch-time Gemini `modelConfigs` / `modelIdResolutions` / `modelChains` are projected into the Codex catalog and runtime fallback snapshot when configured. The injected catalog exposes Gemini reasoning efforts with the 2.5 default thinking budget of 8192 where budget mode is used. `prodex quota` reads the same Google OAuth profile and fetches Gemini Code Assist `retrieveUserQuota` bucket data. Available Super optimizer tools remain local Prodex overlay additions around Codex on this path.

`prodex s gemini --cli gemini` launches the native Google Gemini CLI instead of Codex, defaults its native tools to YOLO approval mode, and routes Code Assist requests through Prodex OAuth profile routing. This native CLI path currently requires a Google OAuth profile and does not accept `--api-key`. Set `PRODEX_GEMINI_BIN` to override the `gemini` executable.

`prodex s gemini --cli agy` launches the native Antigravity CLI with `--dangerously-skip-permissions` so tool permission prompts are auto-approved. Antigravity CLI owns its authentication through the system keyring/Google Sign-In and does not expose an endpoint or token override, so Prodex account auto-rotation and Presidio proxying are not available on this path. It works without a Prodex profile and rejects `--presidio` instead of silently ignoring it. Set `PRODEX_AGY_BIN` to override the `agy` executable.

The OAuth bridge also maps native Gemini `computerUse`, code execution, grounding/citation/URL-context metadata, generated images, video metadata, multimodal file inputs, log-probability metadata, tool-use and cached-token accounting, safety metadata, and Gemini finish reasons into Codex-compatible request, response, and SSE shapes. Citations are emitted as a separate completed output item after Gemini supplies a finish reason. Assistant followups retain native Gemini code, media, video, cache, and thought-signature parts without replaying citation display text as model history.

`@path` and bounded `read_many_files` context honor default binary/build/dependency exclusions plus ordered root `.gitignore`, `.geminiignore`, and custom ignore files, including later negation overrides. Large tool outputs are masked before replaying them into Gemini history and are written to `PRODEX_GEMINI_TOOL_OUTPUT_DIR` or the OS temp directory; set `PRODEX_GEMINI_TOOL_OUTPUT_MASK_THRESHOLD=0` to disable this guard. Codex `/responses/compact` requests use a tool-free unary Gemini semantic-compaction turn on the Gemini CLI `chat-compression-default` alias and return Codex replacement history; a bounded deterministic local summary is used only when semantic compaction fails before commit. Invalid pre-commit Gemini streams are retried with bounded backoff before model fallback.

Gemini CLI compatibility helpers accept inline `gemini_memory` / `gemini_policy` / `gemini_session` request metadata, file-based `gemini_*_file` imports, and `PRODEX_GEMINI_SESSION_FILE` or `PRODEX_GEMINI_CHECKPOINT_FILE` import paths. Gemini memory is loaded by default from `~/.gemini/GEMINI.md`, ancestor `GEMINI.md` files, `.gemini/memory/MEMORY.md`, and `.gemini/memory/INBOX.md`; set `PRODEX_GEMINI_DISABLE_MEMORY=1`, `PRODEX_GEMINI_DISABLE_CONTEXT_FILES=1`, or request metadata `gemini_load_memory=false` to opt out. Gemini settings are read in CLI precedence order from system defaults, global, ancestor project, cwd-local, and system override settings, honoring `GEMINI_CLI_HOME`, `GEMINI_CLI_SYSTEM_SETTINGS_PATH`, and `GEMINI_CLI_SYSTEM_DEFAULTS_PATH`; extension manifests and extension policy TOML files are also read when present to apply Gemini tool allow/exclude, hard command-specific tool-call blocking, and `defaultApprovalMode` behavior.

Before Codex launches, the Gemini provider projects Gemini CLI settings and extension surfaces into the active `CODEX_HOME`: system/global/project `mcpServers` and extension `mcpServers` become generated Codex `[mcp_servers.gemini_*]` entries with settings taking precedence over extension servers of the same Gemini name; system/global/project and extension command hooks are merged into `hooks.json` for Codex `/hooks` review; `~/.gemini/commands`, project `.gemini/commands`, and extension `commands/*.toml` become Codex custom prompts with Gemini command aliases preserved where possible; extension `skills/*/SKILL.md` are copied into generated Codex skill folders under `.agents/skills`; and extension `agents/*.md` become generated Codex custom agents under `agents/*.toml`. Built-in `/prompts:gemini-refresh`, `/prompts:gemini-memory-show`, `/prompts:gemini-memory-refresh`, `/prompts:gemini-memory-inbox`, `/prompts:gemini-remember`, `/prompts:gemini-checkpoint-create`, `/prompts:gemini-checkpoint-restore`, `/prompts:gemini-checkpoint-export`, and `/prompts:gemini-rewind` cover reload/admin, memory, and checkpoint workflows. Generated helper scripts in `CODEX_HOME/bin` include `prodex-gemini-refresh`, `prodex-gemini-checkpoint-create`, and `prodex-gemini-checkpoint-restore`. Set `PRODEX_GEMINI_EXTENSIONS=none` or an allow-list of extension names to control extension loading, `PRODEX_GEMINI_EXTENSION_DIRS` to add extension roots, or `PRODEX_GEMINI_DISABLE_CLI_COMPAT=1` to skip the launch-time Codex surface projection.

Gemini Live realtime websocket translation remains available for compatible callers and credentialed adapter tests, mapping Codex audio, transcript, text, function-call, function-result, interruption, cancellation, housekeeping, and turn-completion events to and from Gemini `BidiGenerateContent`; one Gemini auth/profile is selected before upgrade and remains fixed for the session. Codex 0.140.0 removed the upstream TUI voice controls, so this bridge should not be treated as a normal Codex TUI voice feature. `PRODEX_GEMINI_LIVE_MODEL` overrides the default Live model, while `PRODEX_GEMINI_LIVE_URL` is available for a custom or test Live endpoint. `prodex doctor --runtime` recognizes provider bridge and Gemini markers such as `local_rewrite_provider_model_fallback`, `local_rewrite_gemini_quota_rotate`, `local_rewrite_gemini_invalid_stream_retry`, and `local_rewrite_gemini_live_error`.

Run `npm run test:gemini-schema` after changing Gemini request, response, SSE, semantic compact, exact-output, tool-schema, or Live translation. Run `PRODEX_LIVE_GEMINI=1 npm run test:gemini-live` for a credentialed end-to-end Gemini adapter smoke request; set `PRODEX_BIN` or `PRODEX_LIVE_GEMINI_MODEL` to override the binary or model. Add `PRODEX_LIVE_GEMINI_EXTENDED=1` for command-output-only, file edit, `apply_patch`, reference-repo clone/inspection, optional-tool update discipline, semantic compact, and explicit `exec resume` checks. Add `PRODEX_LIVE_GEMINI_MCP=1` and/or `PRODEX_LIVE_GEMINI_MULTIMODAL=1` when the local environment should also exercise MCP and image-input paths.

</details>

<details>
<summary>Presidio internals (advanced)</summary>

Super asks once whether to enable Presidio. Empty input or `n` keeps it disabled; use `--presidio` or `--no-presidio` for non-interactive launches.

The runtime reads Analyzer, Anonymizer, language, and `fail_mode` settings from `presidio.toml`:

```toml
enabled = true
analyzer_url = "http://localhost:5002"
anonymizer_url = "http://localhost:5001"
language_mode = "auto"
languages = ["en", "id"]
fail_mode = "open"
timeout_ms = 10000
max_response_bytes = 4194304
max_concurrency = 8
```

Enterprise modes additionally require private/on-prem endpoints or exact
`trusted_hosts` entries. Redirects and environment proxy settings are ignored
so inspected content cannot leave the approved endpoint boundary.

The standard Microsoft Analyzer image is English-only. Indonesian detection requires an Analyzer with Indonesian models and recognizers. Presidio quality depends on that service configuration.

</details>

</details>

<details>
<summary>Claude Code — managed Claude Code state</summary>

```bash
prodex claude -- -p "summarize this repo"
prodex claude caveman
prodex claude caveman -- -p "summarize this repo briefly"
prodex claude --profile second caveman -- -p "review the latest diff briefly"
prodex claude --profile second -- -p --output-format json "show the latest diff"
```

`prodex claude` uses the normal Claude Code flow while keeping state under Prodex-managed configuration.

`prodex claude caveman` enables Caveman for that session while keeping state under the Prodex-managed `CLAUDE_CONFIG_DIR`, not the global `~/.claude`.

`prodex claude` is only supported with the default OpenAI/Codex provider.

</details>

## Harness modes

A harness mode is model-facing request policy for a local provider bridge. Prodex supports `auto`,
`native`, `minimal`, and the explicit evaluation-backed `evaluated` mode. The default `auto` still
resolves conservatively to `native`, so existing launches remain unchanged.

```bash
prodex s --provider anthropic --harness native
prodex s --provider anthropic --model claude-sonnet-4-6 --harness evaluated
prodex s deepseek --harness minimal
prodex s gemini --model gemini-3.1-pro-preview --harness evaluated
prodex super --url http://127.0.0.1:8131 --harness minimal
prodex gateway --provider gemini --harness native
```

<details>
<summary>Exact mode behavior and Evaluated policy boundaries</summary>

Native preserves existing request bytes, headers, responses, and streams. Minimal only prepends a
versioned Prodex instruction to eligible canonical `/v1/responses` inference requests. Evaluated
matches the already-selected provider/model against a versioned catalog; it never chooses or
reroutes a provider/model, and unknown pairs are no-ops at the harness layer.

Current Evaluated policies translate supported Anthropic Responses traffic through native
`/v1/messages` and reversibly map Gemini's canonical `exec_command` tool to
`run_shell_command`, restoring the canonical name in typed buffered and SSE tool calls. Ambiguous
tool aliases and unsupported/lossy Anthropic shapes fail closed instead of silently changing
semantics.

The harness is fixed for the bridge or gateway lifetime. It does not change account affinity,
pre-commit rotation, retries, approvals, tools, or streaming commit semantics. Harness selection
never creates a second agent runtime: Codex still owns its agent loop, tools, sandbox, approvals,
skills, hooks, reconnect behavior, and TUI.

</details>

See [docs/harness-modes.md](./docs/harness-modes.md) for exact scope, diagnostics, evaluation
catalog behavior, and non-goals.

## Profiles

<details>
<summary>Common profile commands</summary>

```bash
prodex profile list
prodex profile add second
prodex profile import-current main
prodex login --profile second
prodex use --profile main
prodex logout --profile main
```

</details>

<details>
<summary>More profile commands</summary>

```bash
prodex profile import copilot
prodex profile import kiro
prodex profile export
prodex profile remove second
prodex profile remove --all
```

Password-protected exports now use the version-2 Argon2id envelope. Imports stay
compatible with existing version-1 PBKDF2 bundles.

Imports require a current-user-owned private bundle below trusted directories.
For an existing Unix bundle, correct its ownership and run `chmod 600 backup.json`,
or re-export it. On Windows, the bundle must have a private current-user owner/DACL.

</details>

## Local model support

<details>
<summary>Run Super mode against a local server</summary>

Prodex can launch Super mode against a local OpenAI-compatible server:

```bash
prodex super --url http://127.0.0.1:8131
```

You can use this with a local server such as `llama-server`.

By default, Prodex:

- injects a temporary `prodex-local` Codex provider
- appends `/v1` when the URL has no path
- disables non-function native tools that local servers commonly reject
- advertises a conservative 16k local context window
- skips quota/proxy routing for that launch

Check local server reachability with:

```bash
prodex quota --all --provider local --base-url http://127.0.0.1:8131/v1 --once
```

The default local model id is:

```bash
unsloth/qwen3.5-35b-a3b
```

Override it with `--model`:

```bash
prodex super --url http://127.0.0.1:8131 --model local/qwen
```

Use `--context-window` and `--auto-compact-token-limit` if your local server is configured with a larger context window.

See [LOCAL.md](./LOCAL.md) for self-hosted model setup and testing.

</details>

## Utilities and diagnostics

<details>
<summary>Utility commands</summary>

```bash
prodex status
prodex status --once
prodex info
prodex log
prodex log stream
prodex log upstream
prodex doctor --install
prodex doctor --runtime
prodex doctor --bundle ./prodex-doctor.json --redacted
prodex setup --dry-run
prodex capability list
prodex context audit
prodex context export 019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9
prodex context compress ~/.codex/AGENTS.md --dry-run
git diff | prodex context compact-output --kind git-diff
```

| Command | Description |
|---|---|
| `prodex info` | Shows provider route/quota shapes plus effective runtime tuning values after environment, policy, and default resolution. |
| `prodex log` | Shows the latest session transcript text plus the latest runtime token event. |
| `prodex log stream` | Follows session/runtime logs and prints transcript text plus token events live. Add `--json` for JSON Lines token events only. |
| `prodex log upstream` | Follows HTTP backend-bound LLM payload snapshots after Prodex processing such as Presidio redaction and Smart Context rewriting. Add `--json` for JSON Lines payload events. WebSocket request payloads are not logged. Snapshots are capped at 128 KiB per payload. |
| `prodex doctor --install` | Adds install and embedded asset checks to doctor output. |
| `prodex doctor --runtime` | Runs runtime diagnostics. |
| `prodex doctor --bundle PATH --redacted` | Writes a shareable JSON diagnostic bundle without stored auth tokens or headers. |
| `prodex setup --dry-run` | Shows setup reconciliation actions without changing files. |
| `prodex capability list` | Lists built-in and optional Prodex capabilities with availability status. |
| `prodex context audit` | Reports approximate token weight for shared instruction and memory files. |
| `prodex context export` | Exports a selected shared Codex session transcript/context into a Markdown file. |
| `prodex context compress` | Compresses Markdown/text context files and writes an `.original.md` backup. |
| `prodex context compact-output` | Compacts copied command output such as `git status`, `git diff`, `rg`, `grep`, `find`, `tree`, or long logs. |

For full policy keys, environment overrides, and runtime log path resolution, see [docs/runtime-policy.md](./docs/runtime-policy.md).

When a support case appears to come from upstream Codex itself, run `codex doctor --json` in the same environment as Prodex. Codex 0.135.0 and newer reports richer environment, Git, terminal, app-server, and thread-inventory diagnostics than Prodex's runtime-proxy doctor owns.

</details>

## Advanced behavior

<details>
<summary>Shared Codex history</summary>

Managed Prodex profiles keep account credentials isolated per profile, but Codex-owned shared state uses the native Codex home by default.

On Unix-like systems, this is usually:

```bash
~/.codex
```

In practice, profile `history.jsonl`, `sessions`, `archived_sessions`, `config.toml`, `managed_config.toml`, `environments.toml`, `.credentials.json`, plugins, skills, app-server plugin state, memory-extension state, remote-control enrollment, and Codex runtime SQLite files such as `state_*`, `goals_*`, `logs_*`, and `memories_*` link to the same Codex home that direct Codex uses.

Codex 0.140.0 defaults CLI auth credentials to the file store, so managed Prodex profiles continue to keep `auth.json` isolated per profile, including OpenAI, API-key, and Bedrock API-key auth JSON. MCP OAuth defaults to Codex `auto`; when it falls back to the file store, `.credentials.json` is shared with direct Codex. OS keyring-backed MCP OAuth credentials remain Codex/OS-owned and are not part of Prodex profile export bundles.

Prodex's `prodex-secret-store` keyring backend uses the native macOS Keychain, Windows Credential Manager, or Linux Secret Service. Select it for Prodex secret-store consumers with `PRODEX_SECRET_BACKEND=keyring` or `[secrets].backend = "keyring"` plus an exact `keyring_service`; unavailable or locked OS stores fail closed without exposing secret values. This setting does not migrate Codex-managed profile `auth.json` files, which remain isolated in each profile home.

Prodex strips dynamic-loader injection variables such as `LD_PRELOAD`, `LD_AUDIT`, `LD_LIBRARY_PATH`, and `DYLD_*` from Codex child processes by default. Set `PRODEX_ALLOW_UNSAFE_CHILD_ENV=1` only when intentionally debugging a custom local runtime environment.

Codex cloud-managed config bundle caches are identity/account scoped and remain profile-local. System-level Codex requirements and managed config files remain owned by upstream Codex and the operating system.

Prodex does not synthesize legacy Codex `[profiles.*]` behavior. File-based Codex profile config selected by `--profile` stays in shared Codex state, while Prodex-owned account selection remains in Prodex profile metadata.

Prodex also leaves packaged Codex runtime resources alone, including Codex 0.136.0 and newer bundled zsh runtime helpers under the Codex package layout. Do not set `zsh_path` through Prodex unless you are intentionally debugging direct Codex config.

This matches direct Codex behavior: logging out or switching accounts does not hide chat history.

Older Prodex state from `$PRODEX_HOME/.codex` is merged into the native Codex home on the next managed-profile launch.

Set `PRODEX_SHARED_CODEX_HOME` only when you intentionally want a different shared Codex root.

</details>

<details>
<summary>Bedrock and custom providers</summary>

Auto-rotate and quota checks apply to supported OpenAI/Codex profiles. `prodex quota` also supports Google Gemini OAuth profiles, Antigravity CLI quota snapshots, Anthropic OAuth profiles, imported Copilot accounts, DeepSeek API-key balances, local OpenAI-compatible health snapshots, and configured custom providers.

If a profile's `config.toml` sets `model_provider` to a non-OpenAI backend such as `amazon-bedrock`, `prodex run` and `prodex caveman` launch Codex directly without quota preflight or the local auto-rotate proxy.

Codex 0.143.0 includes upstream Bedrock catalog entries for `openai.gpt-5.6-sol`, `openai.gpt-5.6-terra`, and `openai.gpt-5.6-luna`, plus Codex-owned `max` reasoning effort handling. Prodex leaves those model IDs and Bedrock service-tier behavior owned by the direct Codex launch.

Bedrock quota, credentials, regions, and provider errors are handled by Codex and the upstream provider, not by Prodex.

`prodex quota` shows the configured provider metadata for those profiles instead of failing the view.

</details>

<details>
<summary>Proxy behavior</summary>

Prodex respects system and environment proxy settings for upstream OpenAI quota, auth, and runtime HTTP by default, including:

```bash
HTTP_PROXY
HTTPS_PROXY
NO_PROXY
```

Runtime WebSocket upstream connections also honor `HTTPS_PROXY` and `https_proxy` via HTTP CONNECT and respect `NO_PROXY` and `no_proxy`.

The local Codex-to-Prodex broker connection always receives `NO_PROXY` entries for:

```bash
127.0.0.1
localhost
::1
```

This prevents a user proxy from intercepting the local runtime proxy.

Use `--no-proxy` on `prodex run`, `prodex caveman`, `prodex super`, or `prodex claude` only when you explicitly want Prodex upstream requests to bypass proxy settings.

</details>

<details>
<summary>Contributor notes</summary>

This repository is a Cargo workspace.

The binary crate stays at the root, while reusable leaf crates live under `crates/` to reduce rebuild scope when those components change.

Contributor testing guidance lives in [docs/testing.md](./docs/testing.md), including the fast/serial split and runtime parallel-safety assumptions.

</details>

## Documentation

- [QUICKSTART.md](./QUICKSTART.md) — longer walkthrough
- [LOCAL.md](./LOCAL.md) — self-hosted local model setup and testing
- [docs/state-model.md](./docs/state-model.md) — state ownership and persistence model
- [docs/runtime-policy.md](./docs/runtime-policy.md) — runtime policy keys, environment overrides, and runtime log path resolution
- [docs/deployment.md](./docs/deployment.md) — Docker Compose scaffold for the standalone gateway
- [docs/harness-modes.md](./docs/harness-modes.md) — Harness Mode semantics, evaluated policies, scope, and diagnostics
- [docs/testing.md](./docs/testing.md) — contributor testing guidance

## Support

If you find `prodex` useful and want to support its development, you can donate here:

[<img src="https://www.paypalobjects.com/en_US/i/btn/btn_donateCC_LG.gif" border="0" alt="Donate with PayPal" />](https://paypal.me/christiandoxa)
