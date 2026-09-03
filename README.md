# prodex

[![Release malware scan](https://github.com/christiandoxa/prodex/actions/workflows/standalone-release.yml/badge.svg?branch=main&event=workflow_dispatch)](https://github.com/christiandoxa/prodex/actions/workflows/standalone-release.yml)
[![GitHub release downloads](https://img.shields.io/github/downloads/christiandoxa/prodex/total?label=release%20downloads)](https://github.com/christiandoxa/prodex/releases)

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
- [ChatGPT MCP expose](#chatgpt-mcp-expose)
- [EXPOSE.md](EXPOSE.md)
- [Commands](#commands)
- [Modes](#modes)
- [Sub-agents](docs/sub-agents.md)
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
| Google Gemini | `prodex s gemini` or `prodex s gemini --cli gemini` | Codex bridge: `GEMINI_API_KEY(S)` / `GOOGLE_API_KEY(S)` / `--api-key`; native CLI: CLI-owned supported auth, including Vertex AI | API-key bridge | Third-party Gemini CLI OAuth reuse is disabled. Legacy OAuth profiles fail with migration guidance instead of falling back. Vertex AI is not emulated by the Codex bridge. |
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

The gateway serves `/v1/responses`, `/v1/chat/completions`, `/v1/embeddings`, `/v1/images/*`, `/v1/audio/*`, `/v1/batches`, `/v1/rerank`, `/v1/a2a`, `/v1/messages`, and `/v1/models` where the selected upstream supports them. It adds `x-prodex-call-id` to responses, writes local request detail plus `gateway_spend` events for both `request` and `response` phases to runtime logs, can export those events to JSONL or HTTP using generic, OTel, Datadog, or Langfuse-shaped payloads, supports catalog-backed policy routing strategies (`fallback`, `round-robin`, `least-busy`, `lowest-cost`, `lowest-latency`, `rpm`, `tpm`, `first`) for model aliases/fallback chains, can enforce static virtual keys with persisted request/spend usage plus model/budget/RPM/TPM limits, supports file, SQLite, Postgres, or Redis-backed gateway admin/usage/ledger/SCIM state, and can apply keyword/model, local PII redaction, Presidio, and external webhook guardrails before calls and on outputs. Admin-token, trusted-proxy SSO, or OIDC/JWT bearer requests can list usage, create generated-token keys, rotate/disable/update/delete admin-managed keys, provision SSO users through SCIM-compatible `/v1/prodex/gateway/scim/v2/Users`, inspect usage at `/v1/prodex/gateway/keys` and `/v1/prodex/gateway/usage`, read recent billing ledger records with response-status/output-token reconciliation at `/v1/prodex/gateway/ledger`, read aggregated billing totals at `/v1/prodex/gateway/ledger/summary`, export billing CSV from `/v1/prodex/gateway/ledger.csv` and `/v1/prodex/gateway/ledger/summary.csv`, scrape Prometheus text metrics at `/v1/prodex/gateway/metrics`, inspect provider adapter contracts at `/v1/prodex/gateway/providers` or offline with `prodex gateway providers --json`, inspect active observability and guardrail configuration at `/v1/prodex/gateway/observability` and `/v1/prodex/gateway/guardrails`, fetch the checked-in gateway OpenAPI document at `/v1/prodex/gateway/openapi.json`, and open the built-in gateway admin dashboard at `/v1/prodex/gateway/admin`; the document describes the gateway's documented routes, not complete upstream OpenAPI or provider semantics, and `/v1/a2a` is separate from local `prodex super --sub-agent` execution. Policy/env-backed keys remain read-only, SCIM users can carry tenant/team/project/user/budget scopes for SSO/OIDC fallback, admin-managed key and SCIM user mutations emit `prodex audit` events, and additional admin-plane tokens can be `admin` or read-only `viewer` with optional virtual-key prefix plus tenant/team/project/user/budget scopes. Configure defaults under `[gateway]` in `policy.toml`; validate provider catalog edits with `node scripts/catalog/provider-catalog-check.mjs`. The generated provider matrix lives in [docs/provider-capabilities.md](./docs/provider-capabilities.md).

The gateway can enforce optional model-aware request constraints under `[gateway.request_constraints]`; compatibility defaults leave enforcement disabled and oversized output requests unchanged. Admin/viewer principals can use the dashboard Route Workbench or `POST /v1/prodex/gateway/routes/explain` to inspect the same bounded planner trace without sending upstream traffic or mutating quota, billing, affinity, circuit, admission, or persisted runtime state. Explain payloads and prompt content are not logged or stored.

Gateway virtual-key realtime WebSocket sessions use bounded 32 KiB frames, a five-minute session ceiling, an upfront token reservation, per-frame accounting, and terminal usage reconciliation. One governed provider/profile is fixed before upgrade; policy denial or token exhaustion closes the session without rotation.

Governance policy/classification/provider-registry lifecycle endpoints require SQLite or PostgreSQL state. File and Redis remain compatibility backends for the documented admin/usage surfaces and return `501 governance_policy_operation_unsupported` for governance lifecycle operations; bank mode requires PostgreSQL.

Enterprise OTLP export endpoints from `OTEL_EXPORTER_OTLP_LOGS_ENDPOINT` or `OTEL_EXPORTER_OTLP_ENDPOINT` must be absolute `http://` or `https://` URLs without whitespace, userinfo, query strings, or fragments; put collector credentials in `OTEL_EXPORTER_OTLP_HEADERS`. `prodex-control-plane plan-http-control-plane` request files must use the top-level `principal` field and non-credential HTTP headers, because `Authorization` headers are rejected.

`[gateway.adaptive_routing]` is opt-in. With `enabled = true`, Prodex keeps bounded per-model outcome/latency windows and can reorder only fresh pre-commit alias fallbacks; `shadow_mode = true` records the recommendation without changing selection. Hard continuation affinity, quota filtering, and the no-mid-stream-rotation boundary remain authoritative. Optional `exploration_rate` is deterministic per request and bounded from `0.0` to `1.0`.

JavaScript clients can use `@christiandoxa/prodex-gateway-sdk` for `/v1/responses` plus gateway key, usage, billing ledger, metrics, and OpenAPI admin calls.

</details>

<details>
<summary>Provider behavior details (advanced)</summary>

The auto-rotate proxy is intentionally conservative. It rotates only before a request or stream is committed, preserves `previous_response_id`, turn-state, and session affinity, and does not rotate mid-stream. For fresh work, an explicit account-quota failure excludes that profile and continues through every remaining eligible profile before the final upstream quota response is exposed. Pre-commit transport exhaustion tries the same finite pool before returning local `503 service_unavailable`; generic `429` responses, hard-affinity continuations, and committed output are never replayed merely to hide an error. A provider-wide overload remains retryable with bounded backoff and cancellation/resource limits; it is not reported as quota exhaustion. OpenAI capacity is model-aware: an explicitly labelled Luna Reserve bucket is applicable only to Luna, and when all usable Luna capacity is unavailable, a Luna workflow may continue at a safe pre-commit boundary with the actual `gpt-5.3-codex-spark` model. Prodex keeps requested and effective model state separate and never substitutes Spark for Sol or Terra. Prodex does not auto-redeem reset credits by default. If you launch an OpenAI/Codex runtime path with `--auto-redeem`, Prodex may redeem one earned reset credit only when the weekly usage-limit window is exhausted, no other profile in the quota pool still has weekly quota remaining, and the weekly reset is not already imminent, then retries the same profile before rotating. It still does not redeem for merely critical/thin windows or 5h-only exhaustion. You can also run `prodex redeem <profile>` to send one explicit reset-credit consume request for a named OpenAI/Codex profile; the upstream backend decides whether that manual request applies, reports nothing-to-reset, or reports no-credit/already-redeemed. OpenAI/Codex remains the default quota-aware pool. Antigravity CLI, imported Copilot profiles, Anthropic OAuth profiles, DeepSeek API keys, local OpenAI-compatible URLs, and Bedrock/custom Codex providers have `prodex quota` views. Anthropic, DeepSeek, API-key Gemini, API-key Copilot, Antigravity CLI, local URLs, and Bedrock/custom Codex providers skip OpenAI quota preflight.

Runtime proxy design contract:

- Prodex stays a scoped Codex gateway, not a general-purpose LLM SDK.
- Profile selection must be visible through policy, `prodex info`, `prodex doctor`, and runtime logs.
- Pre-commit retry and fallback paths must stay bounded per request.
- Runtime hot paths must avoid broad disk reads, quota probes, or blocking state saves.
- Quota, budget, transport, and local pressure signals must stay classified separately.
- Selection, admission, affinity, backoff, and first-chunk events must be structured in runtime logs.
- Upstream HTTP/WebSocket connection reuse should be preserved where it does not change Codex semantics.
- Secrets remain profile-isolated and redacted in diagnostics. Prodex-owned mutations emit local audit events; immutable compliance retention, break-glass evidence, SIEM durability, and disaster recovery remain deployment/governance responsibilities. Local audit events and Super/sub-agent overlays do not prove those deployment controls.

</details>

## Installation

These commands install the latest published release.

macOS or Linux:

```sh
curl -fsSL https://github.com/christiandoxa/prodex/releases/latest/download/install.sh | sh
```

Windows PowerShell:

```powershell
irm https://github.com/christiandoxa/prodex/releases/latest/download/install.ps1 | iex
```

Managed profiles require Windows symbolic-link permission. Enable Developer
Mode, grant `SeCreateSymbolicLinkPrivilege`, or run Prodex as an administrator
before importing or creating a profile. Prodex checks this before migrating
profile state and reports an actionable error when permission is unavailable.

<details>
<summary>Installer verification and legacy migration</summary>

Both installers verify their downloaded assets against the release `SHA256SUMS` file. The release workflow malware-scans final assets and verifies installer provenance.

Set `PRODEX_INSTALL_DIR` to choose another binary directory. Standalone installs use the `codex` command on `PATH`; install Codex first if it is not already available. Existing npm or Cargo installations can run `prodex update` once to migrate.

npm and Cargo installations are no longer supported; contributors should use normal workspace commands such as `cargo build` instead of treating a source build as a supported installation channel.

</details>

## Optional tools

Prodex Super keeps a deliberately small optional stack:

- [Caveman](https://github.com/JuliusBrussee/caveman) for an optional response-style plugin.
- [RTK](https://github.com/rtk-ai/rtk) for noisy shell output.
- [Codebase Memory MCP](https://github.com/DeusData/codebase-memory-mcp) for structural code navigation.
- [Playwright MCP](https://github.com/microsoft/playwright-mcp) for browser inspection and automation.
- [Ponytail](https://github.com/DietrichGebert/ponytail) for minimal-implementation guidance.
- [Presidio](https://github.com/data-privacy-stack/presidio) for opt-in PII redaction.

Caveman is externally installed and validated; Smart Context is built into the Codex runtime proxy, not guaranteed for native opaque CLIs. Every default Codex-based `prodex s` or `prodex playwright` launch adds a pinned Playwright MCP server to its temporary overlay when Node.js 18+ and `npx` pass launch-time path validation. Prodex runs without every external tool above; missing tools are skipped instead of blocking Super. See [Optional Tools](docs/optional-tools.md) for pinned Caveman/Ponytail metadata, managed paths, validation, and strict launch behavior.

<details>
<summary>Install and verify the Super tools</summary>

Caveman (Prodex-vetted `2.5.0` checkout):

```bash
export PRODEX_OPTIMIZERS_HOME="${PRODEX_OPTIMIZERS_HOME:-${XDG_DATA_HOME:-$HOME/.local/share}/prodex-optimizers}"
install -d "$PRODEX_OPTIMIZERS_HOME/caveman"
git clone --no-checkout https://github.com/JuliusBrussee/caveman \
  "$PRODEX_OPTIMIZERS_HOME/caveman/2.5.0"
git -C "$PRODEX_OPTIMIZERS_HOME/caveman/2.5.0" config core.autocrlf false
git -C "$PRODEX_OPTIMIZERS_HOME/caveman/2.5.0" checkout --detach \
  f9a039a93d249f3fcdb47a9e02544cd1ce37ba4a
cat >"$PRODEX_OPTIMIZERS_HOME/caveman/2.5.0/prodex-tool.json" <<'JSON'
{
  "schema_version": 1,
  "id": "caveman",
  "version": "2.5.0",
  "source": "https://github.com/JuliusBrussee/caveman",
  "commit": "f9a039a93d249f3fcdb47a9e02544cd1ce37ba4a",
  "tree_sha256": "7b7a90ab252a09f200ea46ad719ad52d36c0b3fbdcbf33769fd7c872f9548dda"
}
JSON

prodex capability super-doctor --json
prodex caveman --dry-run
```

The target directory must not already exist. Prodex validates the commit metadata and complete tree digest before activating Caveman.

RTK (latest stable `0.47.0`, externally managed):

```bash
brew install rtk
# or

rtk --version
rtk gain
prodex capability super-doctor
```

Codebase Memory MCP (latest stable `0.10.8`):

```bash
cbm_install_dir="$(mktemp -d)"
trap 'rm -rf "$cbm_install_dir"' EXIT
curl -fsSLo "$cbm_install_dir/install.sh" \
  https://raw.githubusercontent.com/DeusData/codebase-memory-mcp/v0.10.8/install.sh
CBM_DOWNLOAD_URL=https://github.com/DeusData/codebase-memory-mcp/releases/download/v0.10.8 \
  bash "$cbm_install_dir/install.sh" --skip-config
codebase-memory-mcp daemon status || true
prodex capability super-doctor
```

Prodex accepts daemon-capable Codebase Memory MCP builds (`0.9.1-rc.1` or newer, plus development
builds) so parallel Codex sessions share one coordination daemon, indexing jobs, watchers, and cache.
A separate lightweight stdio frontend per Codex process is expected; `daemon status` exits nonzero
with `daemon: not running` before the first session starts it. Legacy builds that would duplicate
heavy indexing work are skipped unless updated. Prodex leaves `CBM_CACHE_DIR` unset so parent and
sub-agent sessions join the account-wide canonical daemon; an explicit user override is inherited
unchanged and must stay consistent across every CBM client.

Playwright MCP (latest stable, pinned `@playwright/mcp@0.0.80`):

```bash
node --version
npx --version
npx -y @playwright/mcp@0.0.80 install-browser chrome
npx -y @playwright/mcp@0.0.80 --version
prodex capability super-doctor
prodex playwright --dry-run
```

The browser install command above installs the Chrome channel used by Prodex's default MCP configuration. On Linux hosts missing browser system libraries, rerun it with `--with-deps`. Playwright starts through `npx` in headless, isolated mode, so concurrent Prodex terminals do not share browser login state. It prompts before tools marked as writes. Playwright MCP is not a security boundary.

Prodex preserves inherited `[mcp_servers.playwright]` entries. Add a custom entry to the base profile's `config.toml` to change flags, use a persistent/headed browser, or set `enabled = false`; the temporary Super overlay will not replace it.

Ponytail (Prodex-vetted `4.9.0` checkout):

```bash
export PRODEX_OPTIMIZERS_HOME="${PRODEX_OPTIMIZERS_HOME:-${XDG_DATA_HOME:-$HOME/.local/share}/prodex-optimizers}"
install -d "$PRODEX_OPTIMIZERS_HOME/ponytail"
git clone --no-checkout https://github.com/DietrichGebert/ponytail \
  "$PRODEX_OPTIMIZERS_HOME/ponytail/4.9.0"
git -C "$PRODEX_OPTIMIZERS_HOME/ponytail/4.9.0" config core.autocrlf false
git -C "$PRODEX_OPTIMIZERS_HOME/ponytail/4.9.0" checkout --detach \
  0a4dd63ad4541f4f655c4108a295916f3c1d8fda
cat >"$PRODEX_OPTIMIZERS_HOME/ponytail/4.9.0/prodex-tool.json" <<'JSON'
{
  "schema_version": 1,
  "id": "ponytail",
  "version": "4.9.0",
  "source": "https://github.com/DietrichGebert/ponytail",
  "commit": "0a4dd63ad4541f4f655c4108a295916f3c1d8fda",
  "tree_sha256": "88c6dfa10bc0a63385a8f3f01bc4a3e51963c8fd76a0ebc0426bd889f0705970"
}
JSON

prodex capability super-doctor --json
prodex ponytail --dry-run
```

The target directory must not already exist. Prodex validates the commit metadata and complete tree digest, then activates Ponytail only in the temporary overlay for that session. The base Codex profile remains unchanged.

Presidio English services:

```bash
docker run -d --name presidio-analyzer \
  --label com.prodex.presidio.managed=true \
  --label com.prodex.presidio.service=presidio-analyzer \
  -p 127.0.0.1:5002:3000 \
  ghcr.io/data-privacy-stack/presidio-analyzer:2.2.364@sha256:ae8f6f111ac2f04e3fec552f7f80edd0dcbfa2dd69ee1b9e030475be31669885
docker run -d --name presidio-anonymizer \
  --label com.prodex.presidio.managed=true \
  --label com.prodex.presidio.service=presidio-anonymizer \
  -p 127.0.0.1:5001:3000 \
  ghcr.io/data-privacy-stack/presidio-anonymizer:2.2.364@sha256:e567013893ebc80994e3799f6f55c86aa1f0b0fadb779571ab346f0ec45365c1
prodex presidio enable --language-mode fixed --languages en
prodex presidio doctor --json
```

The standard Analyzer image is English-only. Indonesian detection requires an Analyzer configured with Indonesian NLP models and recognizers before enabling `--language-mode auto --languages en,id`.

Verify the optional stack:

```bash
prodex capability super-doctor --presidio --strict
prodex s --no-presidio --dry-run
prodex s --presidio --dry-run
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
prodex login second
prodex login --with-claude
prodex login --with-antigravity
```

Interactive `prodex login` asks for the login method before starting a browser. Choose ChatGPT browser login, device-code login, API-key login, Claude sign-in through Claude Code OAuth, or Antigravity CLI sign-in through `agy auth login`. The Codex-fronted Gemini bridge uses an API key; supported Vertex AI authentication belongs only to the native Gemini CLI. Antigravity login is global to the `agy` CLI and does not create a Prodex profile. For API-key profiles, you can also set an OpenAI-compatible backend URL:

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
- Smart Context Autopilot on Codex/provider-bridge paths.
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

Select or require tools explicitly when needed:

```bash
prodex super --tool caveman --require-tool rtk
prodex super --presidio
```

Interactive Super launches render a terminal Presidio opt-in screen. Pass `--presidio` or `--no-presidio` for non-interactive launches. Ordinary interactive `prodex s` still selects the main agent and provider, then reuses the remembered provider-scoped model and effort without reopening those pickers. Super is the explicit YOLO entrypoint: it launches Codex with approval and sandbox bypass, bypasses hook-trust confirmation, and trusts the current workspace for that invocation without changing persisted Codex configuration. Use `prodex run` for the normal approval and workspace-trust flow.

`prodex s expose` and `prodex s expose --no-tunnel` keep the browser terminal
and MCP route local. `prodex s expose --tunnel` or
`--tunnel-provider cloudflare-quick` (alias `cloudflare`) selects the Cloudflare
Quick Tunnel and publishes the browser terminal plus MCP route.
`--tunnel-provider cloudflare-existing` uses an existing Cloudflare config or
token file. `--tunnel-provider openai` uses the OpenAI Secure MCP Tunnel for MCP
only; the browser remains local and no public browser URL exists. Local MCP and
tunnel-client `/readyz` readiness do not prove ChatGPT connector creation or
remote discovery; the connector remains unverified until real remote traffic
is observed.

On Codex/provider-bridge paths, Smart Context preserves continuation metadata and critical signals while applying deterministic, validated context rewriting. Native opaque CLIs are outside that rewrite boundary. See [docs/smart-context.md](docs/smart-context.md) for its safety model and rollout controls.

Managed optimizer roots are checked in this order: `PRODEX_OPTIMIZERS_HOME`, `$XDG_DATA_HOME/prodex-optimizers`, then `~/.local/share/prodex-optimizers`.

</details>

## OpenAI profile diagnostic

`prodex ping openai` snapshots every configured eligible OpenAI profile and
sends the minimal user text `ping` through the normal Prodex OpenAI/Codex
request path, pinning each probe so one account cannot hide behind another.
It reports each completed response or typed failure, continues after failures,
and exits non-zero unless every requested profile succeeds. A valid completed
model response is enough; the response does not need to say `pong`. Use
`prodex ping openai --profile NAME` for one explicit profile or
`prodex ping openai --json` for the aggregate machine-readable result. This
is an application-level provider diagnostic, not ICMP, DNS, TCP, TLS, or
`/models` connectivity testing.

## ChatGPT MCP expose

Run this from the workspace you want the local connection to start in:

```bash
prodex s expose
```

In a real TTY, `prodex s expose` opens a Ratatui picker with **Local only**
selected first. The choices are Local only, Cloudflare Quick Tunnel, Existing
Cloudflare Tunnel, and OpenAI Secure MCP Tunnel. Use the keyboard to choose and
confirm; Esc or `q` cancels before any external child starts. Without a TTY,
the picker is never opened and the bare command remains loopback-only.

Explicit automation modes are:

```bash
prodex s expose --no-tunnel                      # local browser + local MCP
prodex s expose --tunnel                         # Cloudflare Quick public browser + MCP
prodex s expose --tunnel-provider cloudflare-quick    # same explicit Quick Tunnel mode
prodex s expose --tunnel-provider cloudflare-existing # existing Cloudflare config/token file
prodex s expose --tunnel-provider openai         # OpenAI remote MCP; browser local
```

`cloudflare` remains an alias for `cloudflare-quick`, and `--no-tunnel` remains
the local-only compatibility alias.

WARNING: Cloudflare mode publishes a full-access Super capability. Anyone who
obtains the complete URL can control the expose process as the current OS user;
the URL is not OAuth or multi-user authentication. Existing Cloudflare mode
requires a pre-created user-managed tunnel whose configured hostname routes to
the selected loopback origin; Prodex does not request or display its secrets.
OpenAI Secure MCP Tunnel is MCP-only and never publishes a generic
browser-terminal URL. The OpenAI Platform tunnel must have the intended
ChatGPT workspace association, and the runtime principal needs Tunnels
**Read** + **Use**. A newly created tunnel may need its documented propagation
window before connector setup succeeds.

See [EXPOSE.md](EXPOSE.md) for the verified CLI reference, readiness layers,
Cloudflare QUIC/HTTP/2 and DNS/DoH troubleshooting, OpenAI Secure MCP Tunnel
configuration, lifecycle, security model, and focused tests.

<details>
<summary><strong>Expose configuration and ChatGPT connection</strong></summary>

In an interactive terminal, Prodex asks for the main agent/provider/model,
model-aware effort, optional sub-agent configuration, and expose mode before
starting. Existing Cloudflare setup uses a public hostname and loopback origin
port already configured by the user; Quick Tunnel remains ephemeral. Non-TTY
launches use explicit, remembered, and normal default values without waiting for
stdin.

After readiness, Prodex prints the relevant local/public URLs and provider
status. The interactive Ready panel wraps complete long URLs—including
capability paths—across rows and scrolls its body on short terminals; it never
silently clips or ellipsizes them. Add a model/effort override when needed:

```bash
prodex s expose --model gpt-5.6-luna -c 'model_reasoning_effort="max"'
```

The public URL is intended for ChatGPT Developer Mode's public MCP server URL
connection. Treat the complete URL as a credential and stop the process to
revoke it.

When a plain `prodex s` is already running in the same workspace, the default
MCP tool list also includes `prodex_session_prompt_inject` and
`prodex_session_output_read`. The first accepts only `{ "message": "..." }`
in the common case; the second returns bounded, cursor-based user-visible
assistant/tool output. Both bind to the same process-bound Codex writer and
thread, so this is a no-copy/paste bridge to the existing session and never
starts another solver. The transport is Codex's supported queue/app-server
control plane; it does not write the PTY or SQLite queue payloads.

</details>

## Sub-agents

`prodex s` can delegate bounded tasks to fresh child Prodex processes. Main and
child providers remain separate; Presidio is inherited explicitly; recursion is
disabled; and the official shell-free launcher enforces the active-child limit
across separate launcher processes.

<details>
<summary>Sub-agent configuration, runtime enforcement, and MVP boundaries</summary>

```bash
prodex s --sub-agent --no-presidio
prodex s --sub-agent --sub-agent-max-concurrency default
prodex s --sub-agent --sub-agent-max-concurrency 8
prodex s --presidio --sub-agent --sub-agent-provider kiro \
  --sub-agent-model gpt-5.6-luna \
  --sub-agent-model-reasoning-effort max \
  --sub-agent-max-concurrency 16
prodex s 00000000-0000-7000-8000-000000000042 \
  --sub-agent --sub-agent-max-concurrency=23
```

Interactive fresh-launch order is Presidio, main-agent provider, required main
provider configuration, sub-agent opt-in, then child provider, local URL when
needed, model, catalog-backed or standard effort, and maximum active sub-agents. Answering
no skips every child screen. Explicit parent or child values skip their screens;
non-TTY launches never open a TUI.

The fresh main-agent picker uses the canonical provider registry and offers
OpenAI, Anthropic Claude, GitHub Copilot, DeepSeek, Google Gemini, Kiro, and
Prodex Local. `--provider`, `--url`, an explicit Codex `model_provider`
override, or provider affinity on a resumed session intentionally skips or
locks that picker. In particular, `-c 'model_provider="openai"'` explicitly
selects OpenAI; omit it to choose interactively.

Fresh dry runs report the same remembered provider-scoped model and reasoning
effort as a live launch. Explicit launch values still win, while resumed
threads keep their own persisted settings.

The built-in concurrency default is 4. Presets are 4, 8, 16, and 32; custom
values accept 1 through 64. This limits simultaneous child processes, not total
tasks. When every exclusive lock slot is active, the launcher fails immediately
and tells the main agent to wait for a child before retrying. Child exit, failed
spawn, cancellation, and launcher termination release the OS-backed slot.

The complete instruction block is injected into the temporary effective
`AGENTS.override.md` or `AGENTS.md`; `SUB_AGENTS.md` is diagnostic, not a lone
`@path` reference. Child tasks travel through bounded private task files and an
argument vector built from the current Prodex executable, so quotes, newlines,
Unicode, shell metacharacters, and executable paths with spaces survive without
shell evaluation. Parent UUIDs are never inherited.

OpenAI's canonical picker includes `gpt-5.6-luna` and its `max` effort metadata.
All provider pickers scroll, merge the full offline catalog deterministically,
and keep arbitrary nonempty custom model IDs unchanged.

This remains an MVP delegation surface, not a centralized semantic scheduler.
Prodex does not claim automatic task decomposition, runtime-enforced file
ownership, a global cancellation tree, distributed supervision, A2A child
transport, remote model discovery, or automatic worktree allocation.

See [Sub-agents](docs/sub-agents.md) for the exact CLI, dry-run, instruction,
launcher, concurrency, resume-affinity, and isolation contracts.

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
prodex delete 00000000-0000-7000-8000-000000000042
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
prodex quota
prodex quota --once
prodex quota --provider openai
prodex quota --all
prodex quota --all --once
prodex quota --all --auth no-auth --once
prodex quota --all --provider deepseek --once
prodex quota --all --provider local --base-url http://127.0.0.1:8131/v1 --once
prodex quota --all --provider agy --once
prodex redeem main
prodex gui
prodex s gui
prodex dashboard --open
prodex status
```

Bare `prodex quota` opens the detailed live view across every profile, equivalent to `prodex quota --all --detail`. Provider and auth filters imply that default pool scope, so `prodex quota --provider openai` works without `--all`. Explicit `--all` without `--detail` retains the compact aggregate view; `--profile` and `--raw` retain their single-profile behavior.

`prodex status` opens a btop-inspired live terminal dashboard combining the active/runtime profile, 5-hour and weekly quota/reset/runway, historical token usage and cache efficiency, and aggregate Prodex process CPU, resident memory, disk I/O, and network socket queues. Press `r` to refresh immediately and `q` or `Esc` to exit. `prodex status --once` emits one snapshot for scripts. Resource counters use Linux `/proc`; non-Linux systems show those fields as unavailable while quota and token panels continue working.

The detailed live pool view (`prodex quota` or `prodex quota --all --detail`) accepts `s` to cycle sort modes and `f` to cycle the provider filter through `all`, `openai`, `gemini`, `anthropic`, `copilot`, `kiro`, `deepseek`, `local`, and `agy`. Add `--provider openai`, `--provider gemini`, `--provider anthropic`, `--provider copilot`, `--provider kiro`, `--provider deepseek`, `--provider local`, or `--provider agy` to start locked to a single provider. The table compacts to the current terminal width while preserving status and remaining-quota visibility; its live height keeps the sorted top rows and reports how many profiles are hidden.

For OpenAI/Codex profiles, quota views also show earned rate-limit reset credits when the upstream usage API reports them. Use `prodex redeem <profile>` when you explicitly want to redeem one reset credit on a named profile, even if the 5h and weekly quota windows still have remaining quota. If either quota window resets within 1 hour, Prodex asks before consuming the credit; pass `--yes` to skip that prompt. Add `--auto-redeem` to a runtime launch when you want Prodex to consider a guarded automatic redeem after every OpenAI/Codex profile is weekly-exhausted.

`prodex gui` launches the Codex Desktop interface shown in the [OpenCodex demo](https://github.com/lidge-jun/opencodex/blob/main/assets/demo.gif) through a temporary, profile-scoped `CODEX_HOME` and the Prodex runtime proxy. Chat sessions and the Desktop SQLite index stay shared across all managed Prodex profiles, and launch preflight repairs rollout metadata before Desktop performs its DB-only history query. Existing CLI/Super chats therefore appear regardless of the selected starting profile, and new Desktop chats persist. Source profile configuration and authentication files are not modified. `prodex s gui` launches the same desktop app with the Super/Caveman optimizer overlay and full-access policy. Examples: `prodex gui --profile main` and `prodex s --profile main --no-presidio gui`.

- **macOS and Windows:** run `codex app` and complete installation of the official Codex app. Close any running Codex app before launching it through Prodex so the isolated environment is applied.
- **Linux:** install the `codex-desktop` command from [codex-desktop-linux](https://github.com/ilysenko/codex-desktop-linux). Prodex launches it with `--new-instance`.

Prodex does not download, build, or redistribute either desktop app. Keep the launching terminal open while the GUI runs; that process owns the temporary profile overlay and local proxy.

The browser control plane remains available separately through `prodex dashboard`. Use `prodex dashboard --open` to open it, `prodex dashboard --port 0` for an OS-selected free port, or `--base-url` for quota checks against a custom Codex-compatible backend. The responsive dashboard shows profile/account settings, provider presets, model metadata, quota, a bounded redacted runtime-log tail, theme selection, and runtime/gateway commands. It generates safe provider setup commands instead of storing provider secrets. Prodex enforces a loopback bind; use an SSH tunnel when remote access is required.

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

`prodex update` checks the running version against the latest release before downloading anything. It is a no-op when already current, never automatically downgrades a newer local build, and downloads only a newer checksum-verified GitHub Release binary on macOS, Linux, and Windows. Existing npm or legacy Cargo installations migrate to the standalone path. Update notices state that both legacy installation channels are unsupported and direct them to this command.

</details>

<details>
<summary>More Codex command examples</summary>

```bash
prodex run 00000000-0000-7000-8000-000000000042
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

Codex `rust-v0.153.0` thread classification remains transparent passthrough. `--thread-source` accepts arbitrary upstream feature strings, applies to newly created and forked threads, and is never treated as a Prodex profile or routing signal:

```bash
prodex exec --thread-source automated_review "review this repository"
prodex run exec --thread-source automated_review "review this repository"
prodex caveman exec --thread-source automated_review "review this repository"
prodex s --no-presidio --no-sub-agent exec --thread-source automated_review "review this repository"
prodex exec fork THREAD_ID --thread-source automated_review "continue the review"
```

Prodex preserves the value and its position in Codex argv. It does not inject Codex's `user` default. Resume helpers omit the option so `prodex exec resume THREAD_ID` retains the thread's persisted source. Codex `rust-v0.153.0` enables retained-image budgeting during remote compaction by default; use the normal Codex configuration surface when an explicit override is needed:

```bash
prodex -c features.compaction_image_budget=true exec "review this repository"
prodex run -c features.compaction_image_budget=true exec "review this repository"
prodex caveman -c features.compaction_image_budget=true exec "review this repository"
prodex s --no-presidio --no-sub-agent -c features.compaction_image_budget=true exec "review this repository"
prodex s --provider gemini -c features.compaction_image_budget=true exec "review this repository"
prodex -c features.compaction_image_budget=false exec "review this repository"
```

Generated Prodex provider/default overrides precede passthrough arguments. Explicit user `-c` arguments retain their order, so a later explicit override wins; unspecified state leaves the Codex 0.153.0 default enabled, while explicit `true` or `false` is preserved exactly. Codex owns retained-image accounting, image/label boundary atomicity, and the no-backfill rule. Prodex only preserves the configuration and `/responses/compact` image, label, audio, text, metadata, developer-message, and unknown JSON structures.

Detached memory traffic carries `thread_source: "memory_consolidation"` in `x-codex-turn-metadata` and the matching nested `client_metadata` entry. Prodex preserves both opaquely, including unknown metadata fields and pre-commit retries. This classification is metadata, not a Prodex affinity, selection, rotation, quota, or governance mode. In the explicit JSON-RPC broker, upstream `threadSource` is an optional free-form string on `thread/start` and `thread/fork`; `thread/resume` receives no generated source.

`--web-search` maps to Codex's top-level `web_search = "disabled" | "cached" | "indexed" | "live"` setting. In Super provider mode, an explicit `--web-search` is appended after the provider default, so it overrides the default bridge choice. Codex 0.148.0 allows compatible custom providers to use its standalone search endpoint; Prodex enables that capability automatically for the governed OpenAI-through-Prodex provider. Other custom providers must set `model_providers.<id>.supports_standalone_web_search = true` only when their endpoint implements that contract.

`--rollout-budget-tokens` enables Codex's `[features.rollout_budget]` config. If no reminder thresholds are supplied, Prodex provides valid 75%, 50%, and 25% remaining-token thresholds for the selected limit. Use `--rollout-budget-sampling-weight` and `--rollout-budget-prefill-weight` only when you need Codex's weighted accounting knobs.

`--current-time-reminder` enables Codex's `[features.current_time_reminder]` config. The default system clock source is owned by Codex. `--current-time-clock-source external` is intended for Codex app-server clients that implement the upstream `currentTime/read` request.

`--respect-system-proxy` enables Codex's `[features.respect_system_proxy]` config when the bundled/upstream Codex supports it. Codex 0.148.0 routes auth, model, plugin, MCP, remote-exec, and Responses traffic through its shared proxy-aware HTTP client path so supported system proxy, PAC, WPAD, static proxy, and bypass decisions can be honored. `--no-respect-system-proxy` renders an explicit false override for sessions that need the upstream default direct/env-proxy behavior.

Codex `multiAgentMode` is an app-server/thread setting, not a normal TUI `config.toml` launch override. Prodex therefore does not invent a competing CLI config flag. Launch `prodex app-server` or `prodex run app-server` and pass upstream `multiAgentMode` values (`none`, `explicitRequestOnly`, or `proactive`) through the Codex app-server API.

`prodex mcp-server`, `prodex app-server`, and `prodex exec-server` preserve Codex command-server stdio and protocol arguments. Prodex performs runtime preparation silently: it selects the profile `CODEX_HOME` and routes model HTTP traffic through the same runtime proxy when rotation, pressure controls, or governance require it, without writing launch notices into the protocol stream. The JSON-RPC-aware app-server broker remains an explicit opt-in for validating the stdio frames themselves.

Codex 0.149 commands remain Codex-owned passthrough: `prodex queue --thread THREAD --message TEXT` reaches `codex queue`, and `prodex agents` opens Codex's shared task dashboard. In-session `/cd`, `/pwd`, and `/cwd` are likewise handled by Codex; Prodex consumes turn-time working-directory metadata instead of freezing the launch directory.

`prodex app-server-broker --json` exposes the live-validated broker contract. It recognizes JSON-RPC lifecycle methods such as `initialize`, `thread/start`, `thread/resume`, `thread/fork`, `turn/start`, and `turn/interrupt`, while still accepting compatibility aliases such as `notifications/initialized` and `turn/cancel`. The parser matches upstream wire behavior where `jsonrpc: "2.0"` may be omitted and advertises ordered continuation decisions as `fresh`, `continue-session`, `continue-thread`, and `continue-turn`.

The broker classifies newline-delimited JSON-RPC frames as `batch`, `request`, `notification`, `response`, or `invalid`; bounds stdio reads and rejects lines over 1 MiB before JSON parsing; validates envelope shape, IDs, params, method names, response/error payloads, and lifecycle order; derives session/thread/turn/item metadata plus ordered affinity keys; and exposes invalid-reason counters. Non-empty JSON-RPC batches are bounded to 4,096 members, validated member by member for lifecycle and request/response correlation, and forwarded as the exact original line; empty, oversized, nested, or invalid-member batches fail closed before passthrough. Secret-looking JSON-RPC string fields are redacted from diagnostics and logs. `--experimental-stdio` runs a diagnostic preview, `--experimental-stdio-passthrough-preview` mirrors input with diagnostics on stderr, `--experimental-stdio-validate` fails on invalid input, and `--experimental-stdio-validate-passthrough` forwards only valid frames. `--experimental-stdio-live [--profile NAME]` launches the selected profile's real `codex app-server`, validates both client-to-server and server-to-client streams against one shared lifecycle session, forwards only validated frames, and terminates the child on protocol or transport failure. Default Codex app-server passthrough remains unchanged; the broker does not invent provider routing. Each broker session appends one counts-only local `prodex audit` summary. Schema/replay drift fixtures live under `crates/prodex-app/tests/fixtures/compat_replay/`.

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
prodex caveman 00000000-0000-7000-8000-000000000042
```

`prodex caveman` runs Codex with Caveman mode active in a temporary Prodex overlay `CODEX_HOME`, so the base profile home stays unchanged after the session ends.

Use `--tool rtk`, `--tool playwright`, or `--tool ponytail` to add a session surface. Use `--presidio` for redaction. The `prodex rtk`, `prodex playwright`, and `prodex ponytail` compatibility shortcuts translate to typed selections; tool-like words inside Codex arguments are not removed.

RTK is still an external binary. Install it separately if `rtk gain` is unavailable.

</details>

<details>
<summary>Super mode — daily optional-tool stack</summary>

```bash
prodex s
prodex s exec "review this repo"
prodex s --model gpt-5.3-codex
ANTHROPIC_API_KEY=... prodex s --provider anthropic --model claude-sonnet-4-6
prodex profile import copilot
prodex s --provider copilot --model gpt-5.3-codex
DEEPSEEK_API_KEY=... prodex s deepseek --model deepseek-v4-pro
prodex s gemini
prodex super
prodex super --profile main
prodex super --dry-run
prodex super 00000000-0000-7000-8000-000000000042
```

`prodex s` is the short alias for `prodex super`. `--dry-run` reports the resolved binary, provider, model, profile, proxy mode, and redacted arguments for both Codex-fronted and native `--cli gemini`, `copilot`, `kiro`, and `agy` launches without reading launch credentials or starting the child.
Without `--provider` or `--url`, `--model` selects the standard Codex model. With
either bridge option, it selects that provider's model.

This is my daily mode. It enables validated tools that are installed and launches Codex with Super's approval, sandbox, hook-trust, and workspace-trust bypasses for that invocation.

Playwright MCP is enabled by default in Super when Node.js 18+ and `npx` pass launch-time path validation. Install the pinned package and browser with the commands in [Optional tools](#optional-tools), then use `prodex capability super-doctor` or `--require-tool playwright` for the full offline package probe. Codex Apps are disabled by default in Super; pass a later `-c features.apps=true` override when needed.

Super also enables Smart Context Autopilot on the Codex/provider-bridge path;
native opaque CLIs are not automatically rewritten.

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

`prodex profile import kiro` reads the installed Kiro CLI auth database (`~/.local/share/kiro-cli/data.sqlite3` or the Amazon Q compatibility location when present), snapshots the current credential payload into `kiro_auth.json`, and stores a model catalog snapshot for runtime routing. `--provider kiro` routes Codex through Prodex's local text-only Kiro ACP adapter; ACP owns its tool inventory, while Prodex forwards the prompt, model, and supported reasoning effort and rejects generation controls that ACP cannot enforce. Before Kiro starts, Prodex preserves the shared Codebase Memory MCP server but disables its `check_index_coverage` tool in Kiro's MCP config because that tool's top-level JSON Schema composition is rejected by Kiro/Bedrock; the remaining Codebase Memory tools and canonical account daemon stay shared. The Anthropic Messages compatibility route accepts its required token-limit field but cannot enforce that limit through ACP. `--cli kiro` launches the native Kiro CLI from the imported snapshot and forces its HTTP(S) transport through an authenticated loopback Prodex CONNECT tunnel; `--no-proxy` disables only an outer system proxy. Kiro's proprietary service payload remains end-to-end TLS encrypted, so native Kiro does not gain Smart Context, Presidio, response translation, or account rotation. Native Kiro rejects `--presidio` instead of silently ignoring it. Override binary discovery with `PRODEX_KIRO_BIN` when the installed launcher is not on `PATH`.

Use `--provider deepseek` when you want the Codex/Super front end with DeepSeek as the upstream model:

```bash
prodex s deepseek --model deepseek-v4-pro
```

If `--api-key` is omitted, Prodex reads `DEEPSEEK_API_KEY`; `DEEPSEEK_API_KEYS` may contain multiple comma-, semicolon-, or newline-separated keys for round-robin request rotation and pre-commit retry on auth/quota/rate/temporary failures. This path injects a temporary `prodex-deepseek` Codex provider, exposes a local `/v1/responses` adapter to Codex, forwards ordinary turns to DeepSeek's OpenAI-format chat API, and keeps quota preflight disabled. Prodex also injects a one-model Codex catalog for the selected DeepSeek model, so `/model` stays on that model and offers the DeepSeek-compatible `high`/`xhigh` effort choices. `prodex quota --all --provider deepseek` reads the same `DEEPSEEK_API_KEY(S)` environment and fetches DeepSeek `/user/balance`. Available Super optimizer tools remain local Prodex overlay additions around Codex. `/responses/compact` is handled by a bounded deterministic local summary when the selected provider has no semantic compact implementation.

The DeepSeek catalog includes `deepseek-v4-pro` and `deepseek-v4-flash`; `deepseek-chat` and `deepseek-reasoner` remain compatibility aliases for existing configs.

DeepSeek compatibility is translated, not native Responses. Prodex maps Codex text turns, function/MCP/local shell/apply-patch style tools, `tool_choice`, reasoning effort, JSON object mode, stop sequences, token limits (`max_output_tokens`, `max_tokens`, and `max_completion_tokens`), sampling, logprobs, streaming usage, and DeepSeek cache hit/miss usage into compatible shapes. Request `metadata`, `client_metadata`, `prompt_cache_key`, and `prompt_cache_retention` are preserved in local response metadata instead of being forwarded upstream. JSON schema requests are degraded to DeepSeek `json_object` mode and marked in response metadata because DeepSeek's OpenAI Chat route does not provide native JSON Schema enforcement. Web search is explicit and configurable: `[deepseek] web_search_mode = "auto"` uses DeepSeek's documented Anthropic-compatible `/anthropic/v1/messages` route only when a web-search tool is present, translates function and web-search tools, authenticates with `x-api-key`, and maps search calls, sources, usage, text, and streaming events back to Responses. `"anthropic"` explicitly selects the same native search route, `"off"` rejects web-search tools, and `"openai_chat"` retains best-effort `web_search_options` forwarding with retry fallback. Gateway/profileless launches can use `PRODEX_DEEPSEEK_WEB_SEARCH_MODE`. DeepSeek function tools are bounded by the upstream 128-tool limit; Prodex must fail rather than silently truncate if that ceiling is reached, translated duplicate tool names are rejected instead of being dropped, and named `tool_choice` must target a translated function tool. Tool declarations must also be translatable: function/custom tools require names, namespace tools require a named namespace with named function entries, MCP toolsets that declare inventories require a server name plus allowed/enabled tools, and DeepSeek function names must use only letters, numbers, underscores, or dashes within the upstream 64-character limit. When DeepSeek thinking is enabled, Prodex omits explicit `tool_choice` for upstream compatibility and records the omitted value in DeepSeek response metadata. Reasoning content, refusal text, annotations, logprobs, and finish reasons are preserved in DeepSeek response metadata. Reasoning summaries are not advertised for DeepSeek, and `reasoning.summary` fails clearly. Image, document, audio, and video message content is rejected because this adapter targets DeepSeek's text/tool surfaces. Deprecated `frequency_penalty` and `presence_penalty` are not mapped because current DeepSeek docs mark them unsupported/ignored. `parallel_tool_calls=true` is compatible with DeepSeek's one-or-more tool-call behavior, but `parallel_tool_calls=false` is rejected because the OpenAI Chat route has no equivalent control to enforce it. Responses-only controls that DeepSeek cannot honor, including non-empty `include`, `store=false`, background responses, `truncation=auto`, per-message `cache_control`, `text.verbosity`, legacy `functions`/`function_call`, `logit_bias`, and `max_tool_calls`, fail clearly instead of being dropped.

DeepSeek troubleshooting: if JSON mode appears to stream whitespace or stalls, make sure the prompt explicitly asks for JSON; Prodex adds a minimal adapter instruction when JSON mode is requested, but model-facing task instructions still matter. A DeepSeek 400 in `openai_chat` web-search mode means the best-effort forwarding shape was not accepted; return to `auto`, use `off`, or remove the web-search tool. Strict tool failures usually mean a schema keyword outside DeepSeek's beta strict subset, a missing `required` property, or an object schema that cannot be made `additionalProperties = false`.

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
| Web search | `auto`/`anthropic` use DeepSeek's native Anthropic-compatible web-search tool; `openai_chat` is explicit best-effort forwarding; `off` rejects. |
| Images, documents, audio, video, vision detail | Unsupported on this text/tool adapter; requests fail clearly. |
| Chat prefix completion, FIM `/completions` | Separate DeepSeek beta APIs outside the `/responses` adapter; completion-shaped requests fail fast. |
| Remote compact | Emulated locally with a bounded deterministic summary. |

Strict DeepSeek function calling is opt-in because DeepSeek documents it on the beta endpoint. Add this to the selected Codex profile config:

```toml
[deepseek]
strict_tools = true
beta_base_url = "https://api.deepseek.com/beta"
```

Gateway/profileless launches can use `PRODEX_DEEPSEEK_STRICT_TOOLS=1` and optional `PRODEX_DEEPSEEK_BETA_BASE_URL`. When enabled, Prodex routes rewritten DeepSeek `/responses` traffic through the beta base URL, sets every translated function tool to `strict: true`, forces strict object schemas to require all properties with `additionalProperties = false`, and rejects unsupported strict schema keywords or types clearly. DeepSeek beta chat prefix completion and FIM `/completions` use separate protocols outside this adapter; `prefix`, `prompt`, and `suffix` completion-style requests fail fast instead of being rewritten as chat.

Use `--provider gemini` when you want the Codex/Super front end with Gemini upstream:

```bash
GEMINI_API_KEY=example-key prodex s gemini
prodex s gemini --cli gemini
prodex s gemini --cli agy
GEMINI_API_KEY=example-key prodex s gemini --model gemini-2.5-pro
```

Prodex's third-party Google Gemini OAuth login and Code Assist credential reuse are unsupported and disabled. Existing OAuth profiles remain parseable but fail with guidance to migrate the Codex bridge to a Gemini API key, or to use Vertex AI only through the native Gemini CLI; Prodex never silently falls back. With `--api-key`, `GEMINI_API_KEY(S)`, or `GOOGLE_API_KEY(S)`, Prodex converts Codex Responses requests to Chat Completions and sends them through Google's documented OpenAI-compatible endpoint. Streaming, function calls, continuations, and supported Gemini reasoning values are converted back into Codex Responses semantics. Plural key variables may rotate before commit on auth, quota, rate, or temporary failures. The default model is `auto`; checked-in and configured Gemini model catalogs remain available without a live discovery request.

`prodex s gemini --cli gemini` launches the native Google Gemini CLI instead of Codex and leaves supported authentication, including Vertex AI, and transport to that CLI or environment. Prodex does not inject the removed OAuth client. Set `PRODEX_GEMINI_BIN` to override the `gemini` executable.

`prodex s gemini --cli agy` launches the native Antigravity CLI with `--dangerously-skip-permissions` so tool permission prompts are auto-approved. Antigravity CLI owns its authentication through the system keyring/Google Sign-In and does not expose an endpoint or token override, so Prodex account auto-rotation and Presidio proxying are not available on this path. It works without a Prodex profile and rejects `--presidio` instead of silently ignoring it. Set `PRODEX_AGY_BIN` to override the `agy` executable.

The Gemini bridge also maps native Gemini `computerUse`, code execution, grounding/citation/URL-context metadata, generated images, video metadata, multimodal file inputs, log-probability metadata, tool-use and cached-token accounting, safety metadata, and Gemini finish reasons into Codex-compatible request, response, and SSE shapes. Citations are emitted as a separate completed output item after Gemini supplies a finish reason. Assistant followups retain native Gemini code, media, video, cache, and thought-signature parts without replaying citation display text as model history.

`@path` and bounded `read_many_files` context honor default binary/build/dependency exclusions plus ordered root `.gitignore`, `.geminiignore`, and custom ignore files, including later negation overrides. Large tool outputs are masked before replaying them into Gemini history and are written to `PRODEX_GEMINI_TOOL_OUTPUT_DIR` or the OS temp directory; set `PRODEX_GEMINI_TOOL_OUTPUT_MASK_THRESHOLD=0` to disable this guard. Codex `/responses/compact` requests use a tool-free unary semantic-compaction turn and return Codex replacement history. If semantic compaction fails before commit, Prodex preserves HTTP 200 continuity with a bounded lossy local summary and marks it with `x-prodex-compact-mode: local-fallback`, provider, degraded, and bounded reason headers. Semantic success uses `x-prodex-compact-mode: semantic`. Prometheus output counts both modes and bounded fallback reasons. Invalid pre-commit Gemini streams are retried with bounded backoff before model fallback.

Gemini CLI compatibility helpers accept inline `gemini_memory` / `gemini_policy` / `gemini_session` request metadata, file-based `gemini_*_file` imports, and `PRODEX_GEMINI_SESSION_FILE` or `PRODEX_GEMINI_CHECKPOINT_FILE` import paths. Gemini memory is loaded by default from `~/.gemini/GEMINI.md`, ancestor `GEMINI.md` files, `.gemini/memory/MEMORY.md`, and `.gemini/memory/INBOX.md`; set `PRODEX_GEMINI_DISABLE_MEMORY=1`, `PRODEX_GEMINI_DISABLE_CONTEXT_FILES=1`, or request metadata `gemini_load_memory=false` to opt out. Host-file imports are local-CLI-only: gateway and in-process gateway requests cannot read request-selected or implicit Gemini files from the service host. Gemini settings are read in CLI precedence order from system defaults, global, ancestor project, cwd-local, and system override settings, honoring `GEMINI_CLI_HOME`, `GEMINI_CLI_SYSTEM_SETTINGS_PATH`, and `GEMINI_CLI_SYSTEM_DEFAULTS_PATH`; extension manifests and extension policy TOML files are also read when present to apply Gemini tool allow/exclude, hard command-specific tool-call blocking, and `defaultApprovalMode` behavior.

Before Codex launches, the Gemini provider projects Gemini CLI settings and extension surfaces into the active `CODEX_HOME`: system/global/project `mcpServers` and extension `mcpServers` become generated Codex `[mcp_servers.gemini_*]` entries with settings taking precedence over extension servers of the same Gemini name; system/global/project and extension command hooks are merged into `hooks.json` for Codex `/hooks` review; `~/.gemini/commands`, project `.gemini/commands`, and extension `commands/*.toml` become Codex custom prompts with Gemini command aliases preserved where possible; extension `skills/*/SKILL.md` are copied into generated Codex skill folders under `.agents/skills`; and extension `agents/*.md` become generated Codex custom agents under `agents/*.toml`. Generated files use exact Prodex ownership markers, normalized name collisions receive deterministic numeric suffixes, and rejected existing user files are preserved. Built-in `/prompts:gemini-refresh`, `/prompts:gemini-memory-show`, `/prompts:gemini-memory-refresh`, `/prompts:gemini-memory-inbox`, `/prompts:gemini-remember`, `/prompts:gemini-checkpoint-create`, `/prompts:gemini-checkpoint-restore`, `/prompts:gemini-checkpoint-export`, and `/prompts:gemini-rewind` cover reload/admin, memory, and checkpoint workflows. Generated helper scripts in `CODEX_HOME/bin` include `prodex-gemini-refresh`, `prodex-gemini-checkpoint-create`, and `prodex-gemini-checkpoint-restore`; workspace checkpoints include staged, unstaged, and untracked non-ignored files while excluding the checkpoint directory itself. Set `PRODEX_GEMINI_EXTENSIONS=none` or an allow-list of extension names to control extension loading, `PRODEX_GEMINI_EXTENSION_DIRS` to add extension roots, or `PRODEX_GEMINI_DISABLE_CLI_COMPAT=1` to skip the launch-time Codex surface projection.

Gemini Live realtime websocket translation remains available for anonymous/personal compatible callers, authenticated gateway virtual keys, and credentialed adapter tests, mapping Codex audio, transcript, text, function-call, function-result, interruption, cancellation, housekeeping, and turn-completion events to and from Gemini `BidiGenerateContent`. One governed Gemini auth/profile is selected before upgrade and remains fixed for the bounded session; virtual-key usage is reserved, accounted per text frame, and reconciled when the session ends. Codex 0.140.0 removed the upstream TUI voice controls, so this bridge should not be treated as a normal Codex TUI voice feature. `PRODEX_GEMINI_LIVE_MODEL` overrides the default Live model, while `PRODEX_GEMINI_LIVE_URL` is available for a custom or test Live endpoint. `prodex doctor --runtime` recognizes provider bridge and Gemini markers such as `local_rewrite_provider_model_fallback`, `local_rewrite_gemini_quota_rotate`, `local_rewrite_gemini_invalid_stream_retry`, and `local_rewrite_gemini_live_error`.

Run `cargo test --locked -q -p prodex-provider-core gemini_provider_core_ && cargo test --locked -q -p prodex-app --lib gemini_` after changing Gemini request, response, SSE, semantic compact, exact-output, tool-schema, or Live translation. Run `PRODEX_LIVE_GEMINI=1 node scripts/ci/gemini-live-smoke.mjs` for a credentialed end-to-end Gemini adapter smoke request; set `PRODEX_BIN` or `PRODEX_LIVE_GEMINI_MODEL` to override the binary or model. Add `PRODEX_LIVE_GEMINI_EXTENDED=1` for command-output-only, file edit, `apply_patch`, reference-repo clone/inspection, optional-tool update discipline, semantic compact, and explicit `exec resume` checks. Add `PRODEX_LIVE_GEMINI_MCP=1` and/or `PRODEX_LIVE_GEMINI_MULTIMODAL=1` when the local environment should also exercise MCP and image-input paths.

</details>

<details>
<summary>Presidio internals (advanced)</summary>

Super renders a Ratatui opt-in screen before launch. Empty input or `n` keeps Presidio disabled; use `--presidio` or `--no-presidio` for non-interactive launches.

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

With the default loopback URLs, Prodex auto-starts only the pinned, labeled
containers shown above; set `PRODEX_PRESIDIO_AUTO_START=0` to disable this. It
refuses to start an existing predictable-name container whose ownership label,
image digest, or loopback port binding differs; recreate older `latest`-based
containers explicitly.

The standard Presidio Analyzer image is English-only. Indonesian detection requires an Analyzer with Indonesian models and recognizers. Presidio quality depends on that service configuration.

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

A harness mode is model-facing request policy for a local provider bridge. Prodex supports
`native`, `minimal`, and the explicit evaluation-backed `evaluated` mode. Omitting `--harness`
selects `native`, so existing launches remain unchanged.

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
prodex login second
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
prodex doctor --repair-session-index
prodex setup --dry-run
prodex capability list
prodex context audit
prodex context export 00000000-0000-7000-8000-000000000042
prodex context compress ~/.codex/AGENTS.md --dry-run
git diff | prodex context compact-output --kind git-diff
```

| Command | Description |
|---|---|
| `prodex info` | Shows provider route/quota shapes plus effective runtime tuning values after environment, policy, and default resolution. |
| `prodex log` | Follows the live session/runtime log view; it is the short form of `prodex log stream`. |
| `prodex log stream` | Explicit equivalent of `prodex log`: subscribes to bounded authenticated live runtime sources, including direct and broker-backed proxies, plus session history, printing meaningful assistant/tool/model events plus token events. Routine scheduler `LOAD profile busy` telemetry is excluded before history admission. Its human TUI is titled `Prodex Log`, restores the profile/quota/reset/throughput header, and keeps the last numeric rate visible while idle. Add `--json` for individual JSON Lines events. It does not require a perpetual raw runtime-log journal; legacy recorded files are only a bounded fallback. |
| `prodex log upstream` | Explicit upstream-focused live mode: subscribes to bounded, redacted backend-bound LLM payload snapshots after Prodex processing such as Presidio redaction and Smart Context rewriting. Its human TUI is also titled `Prodex Log`; it never derives t/s from payload bytes and retains the last correlated numeric rate while idle. Add `--json` for JSON Lines payload events. Raw upstream telemetry is not recorded by default; payload snapshots are bounded before broker admission. |
| `prodex doctor --install` | Adds install and embedded asset checks to doctor output. |
| `prodex doctor --runtime` | Runs runtime diagnostics. |
| `prodex doctor --bundle PATH --redacted` | Writes a shareable JSON diagnostic bundle without stored auth tokens or headers. |
| `prodex doctor --repair-session-index` | Explicitly performs full active and archived Codex session-index repair. |
| `prodex setup --dry-run` | Shows setup reconciliation actions without changing files. |
| `prodex capability list` | Lists built-in and optional Prodex capabilities with availability status. |
| `prodex context audit` | Reports approximate token weight for shared instruction and memory files. |
| `prodex context export` | Exports a selected shared Codex session transcript/context into a Markdown file. |
| `prodex context compress` | Compresses Markdown/text context files and writes an `.original.md` backup. |
| `prodex context compact-output` | Compacts copied command output such as `git status`, `git diff`, `rg`, `grep`, `find`, `tree`, or long logs. |

For full policy keys, environment overrides, and runtime log path resolution, see [docs/runtime-policy.md](./docs/runtime-policy.md).

When a support case appears to come from upstream Codex itself, run `prodex run doctor --json` (or `codex doctor --json`) in the same environment as Prodex. Codex 0.149.0 also diagnoses endpoint protection, network/proxy connectivity, desktop state, and update connectivity; those checks remain separate from Prodex's runtime-proxy doctor.

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

Prodex-owned runtime broker capability secrets default to files and can use the native OS keyring instead:

```toml
[secrets]
backend = "keyring"
keyring_service = "prodex"
```

`PRODEX_SECRET_BACKEND` and `PRODEX_SECRET_KEYRING_SERVICE` override policy. The first keyring read migrates an existing broker capability file into an opaque, path-derived keyring account and removes the legacy file only after the keyring write succeeds. `prodex info` and `prodex doctor --runtime --json` report the effective backend. Codex-managed profile `auth.json`, Codex/MCP credentials, and production projected `SecretRef` files retain their existing ownership and are not moved into this keyring.

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

Auto-rotate and quota checks apply to supported OpenAI/Codex profiles. `prodex quota` also supports Antigravity CLI quota snapshots, Anthropic OAuth profiles, imported Copilot accounts, DeepSeek API-key balances, local OpenAI-compatible health snapshots, supported Gemini API-key configurations, and configured custom providers. Disabled Gemini OAuth profiles receive migration guidance.

If a profile's `config.toml` sets `model_provider` to a non-OpenAI backend such as `amazon-bedrock` or Codex 0.148's built-in `amazon-bedrock-runtime`, `prodex run` and `prodex caveman` launch Codex directly without quota preflight or the local auto-rotate proxy. Prodex does not rewrite either provider ID.

Codex 0.149.0 raises the maximum context override for `openai.gpt-5.6-sol`, `openai.gpt-5.6-terra`, and `openai.gpt-5.6-luna` to 872,000 tokens and refreshes expired AWS credentials for Bedrock. Prodex preserves those model-specific limits and leaves AWS refresh, model IDs, reasoning effort, and service-tier behavior owned by the direct Codex launch.

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

- [Documentation index](./docs/README.md) — complete map of maintained documentation
- [QUICKSTART.md](./QUICKSTART.md) — first successful launch
- [LOCAL.md](./LOCAL.md) — self-hosted local model setup and testing
- [docs/state-model.md](./docs/state-model.md) — state ownership and persistence model
- [docs/runtime-policy.md](./docs/runtime-policy.md) — runtime policy keys, environment overrides, and runtime log path resolution
- [docs/deployment.md](./docs/deployment.md) — Docker Compose scaffold for the standalone gateway
- [docs/harness-modes.md](./docs/harness-modes.md) — Harness Mode semantics, evaluated policies, scope, and diagnostics
- [docs/sub-agents.md](./docs/sub-agents.md) — Super sub-agent CLI, child command, and isolation contract
- [docs/testing.md](./docs/testing.md) — contributor testing guidance

## Support

If you find `prodex` useful and want to support its development, you can donate here:

[<img src="https://www.paypalobjects.com/en_US/i/btn/btn_donateCC_LG.gif" border="0" alt="Donate with PayPal" />](https://paypal.me/christiandoxa)
