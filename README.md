# prodex

`prodex` is a multi-account, multi-provider Codex wrapper with quota-aware profile routing.

Use multiple Codex accounts and supported provider backends from one command line. OpenAI/Codex profiles get quota-aware routing and can auto-rotate when multiple eligible profiles exist; provider adapters let `prodex s` launch the Codex front end against Gemini, Anthropic, Copilot, DeepSeek, and local OpenAI-compatible servers.

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
| Codex CLI | `prodex`, `prodex run`, `prodex caveman`, `prodex super` |
| Claude Code | `prodex claude` |
| RTK | `rtk` variants and `prodex s` / `prodex super` |

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
| Google Gemini | `prodex s gemini` | Google OAuth via `prodex login --with-google`, or `GEMINI_API_KEY(S)` / `GOOGLE_API_KEY(S)` / `--api-key` | OAuth profiles | API-key mode uses Google's OpenAI-compatible Chat Completions endpoint; OAuth uses Code Assist. Both can rotate before commit across configured profiles/keys. |
| Google Antigravity CLI | `prodex s gemini --cli agy` | Antigravity keyring / Google Sign-In via `prodex login --with-antigravity` or `agy auth login` | CLI quota snapshot | Native CLI path; no Prodex account auto-rotation or Presidio proxying. |
| Anthropic Claude | `prodex s --provider anthropic` | Claude Code OAuth via `prodex login --with-claude` / `prodex profile import claude`, or `ANTHROPIC_API_KEY(S)` / `--api-key` | OAuth profiles | Shows Claude OAuth readiness; add `ANTHROPIC_ADMIN_KEY` to include Anthropic Admin rate-limit groups. |
| GitHub Copilot | `prodex s --provider copilot` | Imported Copilot CLI profile via `prodex profile import copilot`, or `GITHUB_COPILOT_API_KEY(S)` / `--api-key` | Imported profiles | Native profile and API-key modes can rotate before commit across configured profiles/keys; continuations stay bound to the owning profile. |
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

The gateway serves `/v1/responses`, `/v1/chat/completions`, `/v1/embeddings`, `/v1/images/*`, `/v1/audio/*`, `/v1/batches`, `/v1/rerank`, `/v1/a2a`, `/v1/messages`, and `/v1/models` where the selected upstream supports them. It adds `x-prodex-call-id` to responses, writes local request detail plus `gateway_spend` events for both `request` and `response` phases to runtime logs, can export those events to JSONL or HTTP using generic, OTel, Datadog, or Langfuse-shaped payloads, supports catalog-backed policy routing strategies (`fallback`, `round-robin`, `least-busy`, `lowest-cost`, `lowest-latency`, `rpm`, `tpm`, `first`) for model aliases/fallback chains, can enforce static virtual keys with persisted request/spend usage plus model/budget/RPM/TPM limits, supports file, SQLite, Postgres, or Redis-backed gateway admin/usage/ledger/SCIM state, and can apply keyword/model, local PII redaction, Presidio, and external webhook guardrails before calls and on outputs. Admin-token, trusted-proxy SSO, or OIDC/JWT bearer requests can list usage, create generated-token keys, rotate/disable/update/delete admin-managed keys, provision SSO users through SCIM-compatible `/v1/prodex/gateway/scim/v2/Users`, inspect usage at `/v1/prodex/gateway/keys` and `/v1/prodex/gateway/usage`, read recent billing ledger records with response-status/output-token reconciliation at `/v1/prodex/gateway/ledger`, read aggregated billing totals at `/v1/prodex/gateway/ledger/summary`, export billing CSV from `/v1/prodex/gateway/ledger.csv` and `/v1/prodex/gateway/ledger/summary.csv`, scrape Prometheus text metrics at `/v1/prodex/gateway/metrics`, inspect provider adapter contracts at `/v1/prodex/gateway/providers`, inspect active observability and guardrail configuration at `/v1/prodex/gateway/observability` and `/v1/prodex/gateway/guardrails`, fetch the machine-readable gateway contract at `/v1/prodex/gateway/openapi.json`, and open the built-in gateway admin dashboard at `/v1/prodex/gateway/admin`; policy/env-backed keys remain read-only, SCIM users can carry tenant/team/project/user/budget scopes for SSO/OIDC fallback, admin-managed key and SCIM user mutations are recorded in `prodex audit`, and additional admin-plane tokens can be `admin` or read-only `viewer` with optional virtual-key prefix plus tenant/team/project/user/budget scopes. Configure defaults under `[gateway]` in `policy.toml`; validate provider catalog edits with `npm run catalog:providers`.

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

<details>
<summary>Install from npm</summary>

```bash
npm install -g @christiandoxa/prodex
```

The npm package uses its bundled `@openai/codex@latest` dependency by default. To deliberately use a separate Codex CLI from your machine, set `PRODEX_CODEX_BIN=/path/to/codex` or `PRODEX_CODEX_RESOLUTION=external`.

</details>

<details>
<summary>Install from source checkout</summary>

```bash
cargo install --path .
```

If you install from source, make sure the `codex` binary in your `PATH` is already installed and up to date.

</details>

## Optional tools

`prodex` can run without RTK, SQZ, token-savior, claw-compactor, Presidio, or prodex-memory. `prodex-inspect` is a built-in read-only MCP server for profile/runtime diagnostics and is auto-registered in Prodex overlay sessions. `prodex-memory` is built in and opt-in through the `mem` prefix or managed Mem0 Super prompt.

Install them only if you want to use commands such as:

<details>
<summary>Optional tool commands</summary>

```bash
prodex rtk
prodex sqz
prodex tokensavior
prodex clawcompactor
prodex mem
prodex s doctor
prodex presidio doctor
prodex presidio redact --text "My phone is 212-555-1234"
prodex gateway --provider gemini
prodex s
prodex super
```

</details>

<details>
<summary>Prodex inspect MCP</summary>

`prodex-inspect` exposes read-only MCP tools for agent-side diagnostics:

- `prodex_status`: Prodex paths, active profile, profile count, binding counts, and version.
- `prodex_profiles`: configured profiles without secrets.
- `prodex_latest_runtime_log`: latest runtime log pointer plus a bounded tail excerpt.

Prodex auto-registers it for Prodex overlay sessions. For direct diagnostics:

```bash
prodex __inspect-mcp
```

</details>

<details>
<summary>Prodex memory mode (advanced)</summary>

```bash
prodex s
prodex super
prodex caveman mem
prodex s doctor
```

`prodex s` leaves `prodex-memory` disabled unless you opt in.
The base Codex profile stays unchanged; memory state is owned by Prodex.
Use `prodex caveman mem` when you want only Caveman plus the local memory MCP server, without the full Super stack.

`prodex-memory` provides Mem0-style local memory without Mem0 Cloud, Mem0 CLI, or `MEM0_API_KEY`.
The local `mem` prefix stores memories in a SQLite database under `PRODEX_HOME`:

```text
$PRODEX_HOME/memory/prodex-memory.sqlite
```

`prodex s` asks the Presidio prompt first, then asks whether to enable prodex-memory through managed Mem0 Docker. Empty input or `n` leaves the memory MCP disabled. Answer `y` or pass `--mem0` to start the managed Mem0 OSS Docker server. Use `--no-mem0` to skip that prompt non-interactively, or `prodex caveman mem` / `prodex mem` for the lightweight SQLite backend.

The SQLite path remains the fastest OOTB memory path when you request memory:

- no Mem0 Cloud endpoint
- no `MEM0_API_KEY`
- no user-visible Mem0 auth
- local SQLite storage
- MCP tools exposed as `prodex-memory`

Managed Mem0 mode requires Docker Compose (`docker compose` or `docker-compose`). It does not require a Mem0 Cloud token or a user-supplied provider API key for the default memory path. OpenAI-compatible API-key profiles, `--url` local providers, and supported provider bridges can provide richer upstream LLM/embedding behavior; otherwise Prodex falls back to local embeddings for Mem0.

<details>
<summary>Managed Mem0 internals (advanced)</summary>

Managed Mem0 mode is still local-first:

- Prodex clones `https://github.com/mem0ai/mem0` under `$PRODEX_HOME/mem0` if needed.
- Prodex writes the Mem0 server `.env` with generated local `ADMIN_API_KEY`, `JWT_SECRET`, and Postgres password.
- Mem0 auth stays enabled; Prodex keeps the local API key internal and injects it only into the temporary MCP overlay.
- Mem0 receives `OPENAI_BASE_URL=http://host.docker.internal:<prodex-gateway-port>/v1`.
- Mem0 receives an internal Prodex gateway bearer token as `OPENAI_API_KEY`.
- The gateway selects an efficient memory LLM when available, preferring nano/mini models, and defaults embeddings to `text-embedding-3-small`.
- If no upstream API-key profile is available, the internal gateway still serves deterministic local embeddings for Mem0 and disables generation endpoints for that Mem0-only gateway.
- Prodex writes raw/no-infer memories by default, so the managed path does not spend LLM quota for memory extraction.
- No Mem0 Cloud endpoint and no user-managed `MEM0_API_KEY` are used.

</details>

<details>
<summary>Memory diagnostics (advanced)</summary>

Use `prodex s doctor` to verify that the built-in memory store can be opened before launching Codex. The hidden MCP server can also be launched directly for diagnostics:

```bash
prodex s doctor
prodex __memory-mcp --store /tmp/prodex-memory.sqlite
```

Mem0 Cloud's official MCP endpoint is intentionally not part of Prodex Super's default path. Configure it manually only when you explicitly want Mem0 Platform auth and are willing to manage `MEM0_API_KEY` yourself.

</details>

</details>

<details>
<summary>Install RTK</summary>

RTK is used by the `rtk` variants and by my daily `prodex s` / `prodex super` workflow.

### Homebrew

```bash
brew install rtk-ai/tap/rtk
```

### Linux/macOS quick install

```bash
curl -fsSL https://raw.githubusercontent.com/rtk-ai/rtk/master/install.sh | sh
```

If it installs to `~/.local/bin`, make sure that directory is in your `PATH`:

```bash
echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.bashrc
```

For Zsh:

```bash
echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.zshrc
```

### Cargo

```bash
cargo install --git https://github.com/rtk-ai/rtk rtk
```

### Verify RTK

```bash
rtk --version
rtk gain
```

If `rtk --version` works but `rtk gain` fails, you may have installed a different package named `rtk`.

Remove it and reinstall from the Git URL:

```bash
cargo uninstall rtk
cargo install --git https://github.com/rtk-ai/rtk rtk
```

### Initialize RTK

For Codex:

```bash
rtk init -g --codex
```

For Claude Code:

```bash
rtk init -g
```

Then restart your coding tool.

</details>

<details>
<summary>Install SQZ</summary>

SQZ is used by `prodex sqz` and by Super mode when the `sqz-mcp` binary is available on `PATH` or under a managed optimizer checkout.

Recommended Linux/macOS install:

```bash
curl -fsSL https://raw.githubusercontent.com/ojuschugh1/sqz/main/install.sh | sh
```

Windows PowerShell:

```powershell
irm https://raw.githubusercontent.com/ojuschugh1/sqz/main/install.ps1 | iex
```

Alternative npm install:

```bash
npm install -g sqz-cli
```

Optional source install:

```bash
cargo install sqz-cli
```

Initialize hooks if you also want SQZ outside Prodex:

```bash
sqz init --global
# or only for the current project
sqz init
```

Verify:

```bash
sqz --version
sqz gain
which sqz-mcp
```

Prodex auto-registers `prodex-sqz` for Prodex overlay sessions when `sqz-mcp` is discoverable.

</details>

<details>
<summary>Install token-savior</summary>

token-savior is used by `prodex tokensavior` and by Super mode when the `token-savior` binary is available on `PATH` or under a managed optimizer checkout.

Recommended isolated install:

```bash
git clone https://github.com/Mibayy/token-savior ~/.local/share/prodex-optimizers/token-savior
python3.12 -m venv ~/.local/share/prodex-optimizers/token-savior/.venv
~/.local/share/prodex-optimizers/token-savior/.venv/bin/pip install -e "$HOME/.local/share/prodex-optimizers/token-savior[mcp]"
ln -sf ~/.local/share/prodex-optimizers/token-savior/.venv/bin/token-savior ~/.local/bin/token-savior
```

Use a stable Python interpreter supported by token-savior dependencies, such as Python 3.11, 3.12, or 3.13. Avoid pointing this MCP server at experimental Python releases unless its native dependencies already support them.

Make sure `~/.local/bin` is on `PATH`:

```bash
echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.bashrc
```

If you use Zsh:

```bash
echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.zshrc
```

Verify:

```bash
token-savior --help
which token-savior
```

Prodex handles MCP registration for its own overlay session when it can find the binary, so you do not need to manually edit `.mcp.json` just for `prodex super`.

</details>

<details>
<summary>Install claw-compactor</summary>

claw-compactor is used by `prodex clawcompactor` and by Super mode as a deterministic/local context compaction aid. When discoverable, Super installs a trusted one-shot startup probe that runs `claw-compactor benchmark <workspace> --json` through Prodex's compatibility wrapper and uses a marker under `CODEX_HOME` to avoid replay after Codex conversation restarts. If the workspace has no Markdown memory files, Prodex benchmarks a temporary shadow workspace with a synthetic `MEMORY.md` summary instead of writing into the original directory.

Recommended source install:

```bash
git clone https://github.com/aeromomo/claw-compactor.git ~/.local/share/claw-compactor
python3 -m venv ~/.local/claw-compactor-venv
~/.local/claw-compactor-venv/bin/pip install -e "$HOME/.local/share/claw-compactor[accurate]"
```

If you only need exact token counting for the scripts:

```bash
~/.local/claw-compactor-venv/bin/pip install tiktoken
```

Expose the checkout to Prodex's managed optimizer discovery:

```bash
mkdir -p ~/.local/share/prodex-optimizers
ln -sfn ~/.local/share/claw-compactor ~/.local/share/prodex-optimizers/claw-compactor
```

Quick non-destructive benchmark:

```bash
python3 ~/.local/share/claw-compactor/scripts/mem_compress.py /path/to/workspace benchmark
```

</details>

<details>
<summary>Install Presidio</summary>

Presidio is used by `prodex presidio` and by the optional Super-mode privacy prompt. It runs as local Analyzer and Anonymizer HTTP services.

### English-only quick install

Use Microsoft's published images when you only need English (`en`):

```bash
docker pull mcr.microsoft.com/presidio-analyzer
docker pull mcr.microsoft.com/presidio-anonymizer

docker run -d --name presidio-analyzer -p 5002:3000 mcr.microsoft.com/presidio-analyzer:latest
docker run -d --name presidio-anonymizer -p 5001:3000 mcr.microsoft.com/presidio-anonymizer:latest
```

<details>
<summary>English + Indonesian install (advanced)</summary>

Prodex can route requests with `--language-mode auto --languages en,id`, but Indonesian (`id`) detection only works if the Analyzer supports Indonesian NLP configuration and recognizers. The default Microsoft Analyzer image is usually English-only, so build a custom Analyzer and keep the standard Anonymizer.

Minimal local Analyzer example:

```bash
mkdir -p ~/.local/share/presidio-id-analyzer
cd ~/.local/share/presidio-id-analyzer

cat > Dockerfile <<'EOF'
FROM python:3.11-slim
RUN pip install --no-cache-dir flask gunicorn presidio-analyzer spacy \
 && python -m spacy download en_core_web_sm \
 && python -m spacy download xx_ent_wiki_sm
WORKDIR /app
COPY app.py /app/app.py
CMD ["gunicorn", "-b", "0.0.0.0:3000", "app:app"]
EOF

cat > app.py <<'EOF'
from flask import Flask, jsonify, request
from presidio_analyzer import AnalyzerEngine, Pattern, PatternRecognizer, RecognizerRegistry
from presidio_analyzer.nlp_engine import NlpEngineProvider
from presidio_analyzer.predefined_recognizers import EmailRecognizer, UrlRecognizer

nlp_config = {
    "nlp_engine_name": "spacy",
    "models": [
        {"lang_code": "en", "model_name": "en_core_web_sm"},
        {"lang_code": "id", "model_name": "xx_ent_wiki_sm"},
    ],
}
nlp_engine = NlpEngineProvider(nlp_configuration=nlp_config).create_engine()
registry = RecognizerRegistry(supported_languages=["en", "id"])
registry.load_predefined_recognizers(nlp_engine=nlp_engine, languages=["en", "id"])
registry.add_recognizer(EmailRecognizer(supported_language="id"))
registry.add_recognizer(UrlRecognizer(supported_language="id"))

registry.add_recognizer(PatternRecognizer(
    supported_entity="PHONE_NUMBER",
    language="id",
    patterns=[Pattern("id mobile phone", r"(?<!\d)(?:\+62|62|0)8[1-9][\d\s.-]{7,13}\d(?!\d)", 0.75)],
    context=["telepon", "nomor", "hp", "ponsel"],
))
registry.add_recognizer(PatternRecognizer(
    supported_entity="ID_INDONESIA_NIK",
    language="id",
    patterns=[Pattern("nik", r"(?<!\d)\d{16}(?!\d)", 0.75)],
    context=["nik", "ktp"],
))
registry.add_recognizer(PatternRecognizer(
    supported_entity="ID_INDONESIA_NPWP",
    language="id",
    patterns=[Pattern("npwp", r"(?<!\d)\d{2}\.?\d{3}\.?\d{3}\.?\d[-.]?\d{3}\.?\d{3}(?!\d)", 0.75)],
    context=["npwp", "pajak"],
))
registry.add_recognizer(PatternRecognizer(
    supported_entity="PERSON",
    language="id",
    patterns=[Pattern("nama saya", r"(?i)\b(?:nama saya|saya bernama|nama)\s+[A-Z][A-Za-z.'-]*(?:\s+[A-Z][A-Za-z.'-]*){0,3}", 0.85)],
    context=["nama", "saya", "bernama"],
))

analyzer = AnalyzerEngine(
    nlp_engine=nlp_engine,
    registry=registry,
    supported_languages=["en", "id"],
)
app = Flask(__name__)

@app.get("/health")
def health():
    return jsonify({"status": "ok"})

@app.post("/analyze")
def analyze():
    body = request.get_json(force=True)
    results = analyzer.analyze(
        text=body["text"],
        language=body.get("language", "en"),
        entities=body.get("entities"),
        score_threshold=body.get("score_threshold", 0),
    )
    return jsonify([result.to_dict() for result in results])
EOF

docker build -t prodex-presidio-analyzer-id:latest .
docker rm -f presidio-analyzer presidio-anonymizer 2>/dev/null || true
docker run -d --name presidio-analyzer -p 5002:3000 prodex-presidio-analyzer-id:latest
docker run -d --name presidio-anonymizer -p 5001:3000 mcr.microsoft.com/presidio-anonymizer:latest

prodex presidio enable --language-mode auto --languages en,id
```

This example keeps built-in recognizers such as `EMAIL_ADDRESS` and `URL`, adds Indonesian phone numbers, NIK, NPWP, and context-based `PERSON` detection for text like `Nama saya Budi`.

</details>

### Verify

```bash
prodex presidio doctor --json
prodex presidio redact --language en --text "My name is John Smith and my phone is 212-555-1234."
prodex presidio redact --language id --text "Nama saya Budi dan nomor telepon saya adalah 0812-3456-7890."
prodex presidio redact --language id --text "NIK saya 3171010101900001 dan email saya budi@example.com."
```

English should redact `PERSON` and `PHONE_NUMBER`. Indonesian should redact `PERSON`, `PHONE_NUMBER`, `ID_INDONESIA_NIK`, and `EMAIL_ADDRESS`. If `id` misses names or identifiers, the Analyzer container is not using the custom Indonesian config/recognizers.

### Use with `prodex s`

Once `prodex presidio enable --language-mode auto --languages en,id` is configured and the custom Analyzer plus Anonymizer containers are healthy, run `prodex s` and answer `y` to the Presidio prompt. That is enough to enable runtime request-body and WebSocket text-frame redaction for the session. Use `--presidio` to enable it non-interactively or `--no-presidio` to skip it.

If the endpoints are unhealthy, Prodex may auto-start the default Microsoft containers. Those are English-only unless your custom Analyzer image/container is already configured and running. Set `PRODEX_PRESIDIO_AUTO_START=0` to disable best-effort auto-start and only use configured endpoints.

<details>
<summary>Docker Desktop context note</summary>

If the containers do not appear in Docker Desktop, check the active Docker context. Docker Desktop usually uses `desktop-linux`.

```bash
docker context show
docker context ls
docker --context desktop-linux ps -a --filter name=presidio
```

</details>
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

## Daily command: `prodex s`

<details>
<summary>Runs Codex with Caveman, RTK, and local optimizer guidance</summary>

For daily work, I use:

```bash
prodex s
```

`prodex s` is an alias for:

```bash
prodex super
```

This is the mode I tune and use myself every day.

It combines:

- Caveman mode
- RTK shell-command guidance
- full-access launch mode
- Smart Context Autopilot in the runtime proxy
- deterministic/local accommodation for `sqz`, `token-savior`, and `claw-compactor` low-token workflows

```bash
prodex s
prodex s exec "review this repo"
prodex s expose
```

`prodex super` expands to:

```bash
prodex caveman rtk sqz tokensavior clawcompactor --full-access
```

<details>
<summary>Prompts, Presidio, memory, and expose (advanced)</summary>

Before launch, Super asks whether to add Presidio redaction. Empty input or `n` keeps the expansion above. If you answer `y`, it is equivalent to:

```bash
prodex caveman rtk sqz tokensavior clawcompactor presidio --full-access
```

Use `prodex super --presidio` to enable Presidio without prompting, or `prodex super --no-presidio` to skip the prompt and keep Presidio disabled. Presidio enables runtime request-body and WebSocket text redaction through local Presidio for the session when services are healthy; service failures follow `fail_mode`. The runtime uses `presidio.toml` endpoints when configured, falls back to `http://localhost:5002` and `http://localhost:5001`, and honors `fail_mode = "open"` or `"closed"`.

After the Presidio prompt, Super asks whether to enable prodex-memory through managed Mem0 Docker. Empty input or `n` leaves `prodex-memory` disabled, so Codex will not wait for that MCP server. Answer `y` or pass `--mem0` to start the managed Mem0 OSS Docker server, route its OpenAI-compatible calls through a session-local Prodex gateway, and inject the local Mem0 API key into the temporary Codex MCP config. Use `--no-mem0` to skip the prompt. Use the `mem` optimizer prefix for local SQLite memory. This path does not use Mem0 Cloud or `MEM0_API_KEY`; Docker Compose is required, and Prodex falls back to local embeddings when no upstream provider API key is available.

Super prints prelaunch progress for runtime proxy setup, Presidio auto-start/checks, and managed Mem0 Docker startup. The output happens before Codex starts; runtime notices still go to logs once the TUI is running.

Full access maps to Codex's sandbox-bypass launch flag. Use it only when you intentionally want Codex to run without the normal approval and sandbox protections.

Use `prodex s doctor` to inspect the Super optimizer stack without launching Codex. Add `--json` for machine-readable output, `--strict` to exit non-zero when any optimizer check is unavailable, and `--presidio` to include local Presidio Analyzer/Anonymizer health checks. `prodex s --dry-run` also prints the same optimizer matrix for the launch preview.

Use `prodex s expose` when you need to reach the live Super terminal from a browser. Prodex starts a local PTY bridge protected by a high-entropy access token, launches `cloudflared tunnel --protocol http2 --url ...` when `cloudflared` is available, and prints both the loopback and Cloudflare quick-tunnel URLs. The browser tab can close without stopping the session; reopening the same token URL reconnects to the existing PTY and replays recent scrollback. Add `--no-tunnel` for local-only access, `--max-clients N` to cap simultaneous browsers, or `--command 'prodex s --no-presidio'` to choose the initial terminal command.

</details>

<details>
<summary>Super optimizer internals (advanced)</summary>

Super's built-in optimization stack is deliberately local and deterministic. It preloads Caveman, exposes a Prodex overlay `rtk` PATH wrapper plus RTK auto-wrappers for common noisy commands when RTK is installed, auto-registers built-in `prodex-inspect` plus discovered `sqz-mcp` and `token-savior` MCP servers, exposes `sqz` and `claw-compactor` wrapper commands when those commands/checkouts are discoverable, invokes a trusted one-shot `prodex-claw-compactor-sessionstart` SessionStart benchmark probe when Claw-Compactor is available, falls back to a temporary shadow `MEMORY.md` when the workspace has no Markdown memory files, then uses Smart Context Autopilot through a dedicated runtime proxy for lower-token request shaping. The probe delegates to `prodex-claw-compactor-auto "$(pwd)"` and uses a marker under `CODEX_HOME` so Codex conversation restarts do not replay it. Presidio redaction and prodex-memory are added only when you opt in. Prodex routes optional-tool state under `PRODEX_HOME` (default `~/.prodex`), including SQZ XDG state, token-savior cache/stats, claw-compactor HOME/XDG state, RTK analytics paths, and local memory, so compatible optimizer metadata stays out of worktrees.

Super instructs Codex to use the available local optimizer stack where it fits the task, not just RTK:

- RTK works upstream/input-side. Use visible `rtk <cmd>` for noisy terminal commands before their output enters the model context, such as `git diff`, `cargo test`, `npm test`, build logs, and package-manager output. Prodex also auto-wraps common noisy commands as a fallback when RTK is installed, but that fallback does not make the TUI show an `rtk` prefix.
- SQZ works downstream/context-side through the auto-registered `prodex-sqz` MCP server when `sqz-mcp` is available. Use it for repeated workspace reads, large text blobs, long command outputs that need reuse, and long-session context compression instead of emitting the same full content again.
- token-savior handles symbol lookup, caller/context navigation, duplicate/dead-code checks, and API-impact searches before broad source reads when token-savior is available.
- prodex-inspect provides read-only MCP diagnostics for Prodex status, profiles, and latest runtime log tail.
- claw-compactor handles workspace-level summary or benchmark requests through `prodex-claw-compactor` / `prodex-claw-compactor-auto` when available; treat its output as overview context and reread exact source before edits.
- prodex-memory provides local Mem0-style memory through the `mem` prefix with SQLite, or through managed Mem0 OSS Docker when you opt in with the Super prompt or `--mem0`; neither path uses Mem0 Cloud auth or `MEM0_API_KEY`.
- Presidio stays optional and only runs when you opt in with the Super prompt or `--presidio`.

Managed optimizer checkouts are discovered from `PRODEX_OPTIMIZERS_HOME`, `$XDG_DATA_HOME/prodex-optimizers`, then `~/.local/share/prodex-optimizers`.
The generated `SUPER_OPTIMIZERS.md` overlay includes an `Available Now` section so the model can see which MCP servers and wrappers were actually discovered for that session.

</details>

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

Codex-owned TUI commands such as `/usage`, `/goal`, `/import`, and `/delete` stay upstream Codex behavior. Prodex preserves their request metadata through the proxy and does not add a competing command surface. The CLI form `prodex delete <session>` passes through to Codex and, after a successful delete, prunes matching Prodex session affinity metadata.

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
```

The live `prodex quota --all --detail` view accepts `s` to cycle sort modes and `f` to cycle the provider filter through `all`, `openai`, `gemini`, `anthropic`, `copilot`, `deepseek`, and `local`. Add `--provider openai`, `--provider gemini`, `--provider anthropic`, `--provider copilot`, `--provider deepseek`, or `--provider local` to start locked to a single provider.

For OpenAI/Codex profiles, quota views also show earned rate-limit reset credits when the upstream usage API reports them. Use `prodex redeem <profile>` when you explicitly want to redeem one reset credit on a named profile, even if the 5h and weekly quota windows still have remaining quota. If either quota window resets within 1 hour, Prodex asks before consuming the credit; pass `--yes` to skip that prompt. Add `--auto-redeem` to a runtime launch when you want Prodex to consider a guarded automatic redeem after every OpenAI/Codex profile is weekly-exhausted.

`prodex dashboard` serves a local browser dashboard at `http://127.0.0.1:8765` by default. It shows profile/account settings, lets you switch or remove profile entries, and renders live usage from the same quota collectors used by `prodex quota`. Use `prodex dashboard --port 0` for an OS-selected free port, or pass `--base-url` for quota checks against a custom Codex-compatible backend. The dashboard has no password auth; keep it on localhost unless the network is trusted.

</details>

<details>
<summary>Sessions</summary>

```bash
prodex session list
prodex session current
prodex session current --include-subagents
```

</details>

<details>
<summary>Update Codex</summary>

```bash
prodex update --help
```

`prodex update` passes through to `codex update` directly. It does not use Prodex profile selection, quota preflight, or the local runtime proxy.

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
```

`--web-search` maps to Codex's top-level `web_search = "disabled" | "cached" | "indexed" | "live"` setting. In Super provider mode, an explicit `--web-search` is appended after the provider default, so it overrides the default bridge choice.

`--rollout-budget-tokens` enables Codex's `[features.rollout_budget]` config. If no reminder thresholds are supplied, Prodex provides valid 75%, 50%, and 25% remaining-token thresholds for the selected limit. Use `--rollout-budget-sampling-weight` and `--rollout-budget-prefill-weight` only when you need Codex's weighted accounting knobs.

`--current-time-reminder` enables Codex's `[features.current_time_reminder]` config. The default system clock source is owned by Codex. `--current-time-clock-source external` is intended for Codex app-server clients that implement the upstream `currentTime/read` request.

Codex `multiAgentMode` is an app-server/thread setting, not a normal TUI `config.toml` launch override. Prodex therefore does not invent a competing CLI config flag. Launch `prodex app-server` or `prodex run app-server` and pass upstream `multiAgentMode` values (`none`, `explicitRequestOnly`, or `proactive`) through the Codex app-server API.

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
prodex sqz
prodex tokensavior
prodex clawcompactor
prodex mem
prodex caveman --dry-run
prodex s doctor
prodex s doctor --json --strict
prodex caveman --profile main
prodex caveman exec "review this repo in caveman mode"
prodex caveman 019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9
```

`prodex caveman` runs Codex with Caveman mode active in a temporary Prodex overlay `CODEX_HOME`, so the base profile home stays unchanged after the session ends.

Add optimizer prefixes before Codex args when you want Prodex to enable a specific session tool in the Prodex overlay: `rtk`, `sqz`, `tokensavior`, `clawcompactor`, `mem`, or `presidio`. Top-level shortcuts such as `prodex rtk`, `prodex sqz`, and `prodex mem` map to `prodex caveman <prefix>`.

RTK is still an external binary. Install it separately if `rtk gain` is unavailable.

</details>

<details>
<summary>Super mode — daily Caveman + RTK + optimizer stack</summary>

```bash
prodex s
prodex s exec "review this repo"
ANTHROPIC_API_KEY=... prodex s --provider anthropic --model claude-sonnet-4-6
prodex profile import copilot
prodex s --provider copilot --model gpt-5.1-codex
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
prodex s --provider anthropic --model claude-sonnet-4-6 --api-key "$ANTHROPIC_API_KEY"
```

If `--api-key` is omitted, Prodex uses the Anthropic profile created by `prodex login --with-claude` or `prodex profile import claude`. API-key mode still reads `ANTHROPIC_API_KEY`; `ANTHROPIC_API_KEYS` may contain multiple comma-, semicolon-, or newline-separated keys for round-robin request rotation and pre-commit retry on auth/quota/rate/temporary failures. This path injects a temporary `prodex-anthropic` Codex provider, exposes a local `/v1/responses` adapter to Codex, forwards to Anthropic's OpenAI-compatible chat API, and keeps quota preflight disabled. `prodex quota --all --provider anthropic` shows OAuth readiness for Anthropic profiles; set `ANTHROPIC_ADMIN_KEY` when you want Anthropic Admin rate-limit groups included.

Use `--provider copilot` when you want the Codex/Super front end with GitHub Copilot upstream:

```bash
prodex profile import copilot
prodex s --provider copilot --model gpt-5.1-codex
```

Without `--api-key`, Prodex uses imported Copilot CLI profiles, refreshes Copilot runtime API tokens from GitHub before launch, can rotate fresh native Responses requests across multiple eligible profiles, and binds streaming response IDs back to the owning profile for continuations. `GITHUB_COPILOT_API_KEY`, `GITHUB_COPILOT_API_KEYS`, or `--api-key` can be used when you already have Copilot runtime API token(s); plural keys may be comma-, semicolon-, or newline-separated and can rotate before commit on auth/quota/rate/temporary failures.

Use `--provider deepseek` when you want the Codex/Super front end with DeepSeek as the upstream model:

```bash
prodex s deepseek --model deepseek-v4-pro --api-key "$DEEPSEEK_API_KEY"
```

If `--api-key` is omitted, Prodex reads `DEEPSEEK_API_KEY`; `DEEPSEEK_API_KEYS` may contain multiple comma-, semicolon-, or newline-separated keys for round-robin request rotation and pre-commit retry on auth/quota/rate/temporary failures. This path injects a temporary `prodex-deepseek` Codex provider, exposes a local `/v1/responses` adapter to Codex, forwards to DeepSeek's OpenAI-format chat API, and keeps quota preflight disabled. Prodex also injects a one-model Codex catalog for the selected DeepSeek model, so `/model` stays on that model and offers the DeepSeek-compatible `high`/`xhigh` effort choices. `prodex quota --all --provider deepseek` reads the same `DEEPSEEK_API_KEY(S)` environment and fetches DeepSeek `/user/balance`. Available Super optimizer tools remain local Prodex overlay additions around Codex. Remote compact is not implemented for this adapter yet, so the default DeepSeek context window is large and `--auto-compact-token-limit` defaults high.

Use `--provider gemini` when you want the Codex/Super front end with Gemini upstream:

```bash
prodex login --with-google
prodex s gemini
prodex s gemini --cli gemini
prodex s gemini --cli agy
GEMINI_API_KEY=... prodex s gemini --model gemini-2.5-pro
prodex s gemini --model gemini-2.5-pro --api-key "$GEMINI_API_KEY"
```

Without `--api-key`, Prodex uses the Google OAuth profile created by `prodex login --with-google` or the interactive Google sign-in choice, then routes through Google's Code Assist Gemini endpoint. Google login verifies Code Assist readiness before creating or updating the profile, and may open a second browser page if Google requires account verification. With `--api-key`, or `GEMINI_API_KEY(S)` / `GOOGLE_API_KEY(S)`, Prodex converts Codex Responses requests to Chat Completions and sends them through Google's documented `/v1beta/openai/chat/completions` endpoint with Bearer authentication. Streaming, function calls, continuations, and Gemini `reasoning_effort` values are converted back into Codex Responses semantics. Plural key env vars may be comma-, semicolon-, or newline-separated and can rotate before commit on auth/quota/rate/temporary failures. OAuth sessions keep fresh Gemini requests sticky to the previous successful profile by default for smoother Codex-style continuity; set `PRODEX_GEMINI_STICKY_FRESH_OAUTH=0` to restore pure fresh-request round robin. The default model is `auto`, matching Gemini CLI-style model routing through Gemini 3 and stable fallbacks; launch-time Gemini `modelConfigs` / `modelIdResolutions` / `modelChains` are projected into the Codex catalog and runtime fallback snapshot when configured. The injected catalog exposes Gemini reasoning efforts with the 2.5 default thinking budget of 8192 where budget mode is used. `prodex quota` reads the same Google OAuth profile and fetches Gemini Code Assist `retrieveUserQuota` bucket data. Available Super optimizer tools remain local Prodex overlay additions around Codex on this path.

`prodex s gemini --cli gemini` launches the native Google Gemini CLI instead of Codex, defaults its native tools to YOLO approval mode, and routes Code Assist requests through Prodex OAuth profile routing. This native CLI path currently requires a Google OAuth profile and does not accept `--api-key`. Set `PRODEX_GEMINI_BIN` to override the `gemini` executable.

`prodex s gemini --cli agy` launches the native Antigravity CLI with `--dangerously-skip-permissions` so tool permission prompts are auto-approved. Antigravity CLI owns its authentication through the system keyring/Google Sign-In and does not expose an endpoint or token override, so Prodex account auto-rotation and Presidio proxying are not available on this path. Set `PRODEX_AGY_BIN` to override the `agy` executable.

The OAuth bridge also maps native Gemini `computerUse`, code execution, grounding/citation/URL-context metadata, generated images, video metadata, multimodal file inputs, log-probability metadata, tool-use and cached-token accounting, safety metadata, and Gemini finish reasons into Codex-compatible request, response, and SSE shapes. Citations are emitted as a separate completed output item after Gemini supplies a finish reason. Assistant followups retain native Gemini code, media, video, cache, and thought-signature parts without replaying citation display text as model history.

`@path` and bounded `read_many_files` context honor default binary/build/dependency exclusions plus ordered root `.gitignore`, `.geminiignore`, and custom ignore files, including later negation overrides. Large tool outputs are masked before replaying them into Gemini history and are written to `PRODEX_GEMINI_TOOL_OUTPUT_DIR` or the OS temp directory; set `PRODEX_GEMINI_TOOL_OUTPUT_MASK_THRESHOLD=0` to disable this guard. Codex `/responses/compact` requests use a tool-free unary Gemini semantic-compaction turn on the Gemini CLI `chat-compression-default` alias and return Codex replacement history; a bounded deterministic local summary is used only when semantic compaction fails before commit. Invalid pre-commit Gemini streams are retried with bounded backoff before model fallback.

Gemini CLI compatibility helpers accept inline `gemini_memory` / `gemini_policy` / `gemini_session` request metadata, file-based `gemini_*_file` imports, and `PRODEX_GEMINI_SESSION_FILE` or `PRODEX_GEMINI_CHECKPOINT_FILE` import paths. Gemini memory is loaded by default from `~/.gemini/GEMINI.md`, ancestor `GEMINI.md` files, `.gemini/memory/MEMORY.md`, and `.gemini/memory/INBOX.md`; set `PRODEX_GEMINI_DISABLE_MEMORY=1`, `PRODEX_GEMINI_DISABLE_CONTEXT_FILES=1`, or request metadata `gemini_load_memory=false` to opt out. Gemini settings are read in CLI precedence order from system defaults, global, ancestor project, cwd-local, and system override settings, honoring `GEMINI_CLI_HOME`, `GEMINI_CLI_SYSTEM_SETTINGS_PATH`, and `GEMINI_CLI_SYSTEM_DEFAULTS_PATH`; extension manifests and extension policy TOML files are also read when present to apply Gemini tool allow/exclude, hard command-specific tool-call blocking, and `defaultApprovalMode` behavior.

Before Codex launches, the Gemini provider projects Gemini CLI settings and extension surfaces into the active `CODEX_HOME`: system/global/project `mcpServers` and extension `mcpServers` become generated Codex `[mcp_servers.gemini_*]` entries with settings taking precedence over extension servers of the same Gemini name; system/global/project and extension command hooks are merged into `hooks.json` for Codex `/hooks` review; `~/.gemini/commands`, project `.gemini/commands`, and extension `commands/*.toml` become Codex custom prompts with Gemini command aliases preserved where possible; extension `skills/*/SKILL.md` are copied into generated Codex skill folders under `.agents/skills`; and extension `agents/*.md` become generated Codex custom agents under `agents/*.toml`. Built-in `/prompts:gemini-refresh`, `/prompts:gemini-memory-show`, `/prompts:gemini-memory-refresh`, `/prompts:gemini-memory-inbox`, `/prompts:gemini-remember`, `/prompts:gemini-checkpoint-create`, `/prompts:gemini-checkpoint-restore`, `/prompts:gemini-checkpoint-export`, and `/prompts:gemini-rewind` cover reload/admin, memory, and checkpoint workflows. Generated helper scripts in `CODEX_HOME/bin` include `prodex-gemini-refresh`, `prodex-gemini-checkpoint-create`, and `prodex-gemini-checkpoint-restore`. Set `PRODEX_GEMINI_EXTENSIONS=none` or an allow-list of extension names to control extension loading, `PRODEX_GEMINI_EXTENSION_DIRS` to add extension roots, or `PRODEX_GEMINI_DISABLE_CLI_COMPAT=1` to skip the launch-time Codex surface projection.

Gemini Live realtime websocket translation remains available for compatible callers and credentialed adapter tests, mapping Codex audio, transcript, text, function-call, function-result, interruption, cancellation, housekeeping, and turn-completion events to and from Gemini `BidiGenerateContent`; one Gemini auth/profile is selected before upgrade and remains fixed for the session. Codex 0.140.0 removed the upstream TUI voice controls, so this bridge should not be treated as a normal Codex TUI voice feature. `PRODEX_GEMINI_LIVE_MODEL` overrides the default Live model, while `PRODEX_GEMINI_LIVE_URL` is available for a custom or test Live endpoint. `prodex doctor --runtime` recognizes provider bridge and Gemini markers such as `local_rewrite_provider_model_fallback`, `local_rewrite_gemini_quota_rotate`, `local_rewrite_gemini_invalid_stream_retry`, and `local_rewrite_gemini_live_error`.

Run `npm run test:gemini-schema` after changing Gemini request, response, SSE, semantic compact, exact-output, tool-schema, or Live translation. Run `PRODEX_LIVE_GEMINI=1 npm run test:gemini-live` for a credentialed end-to-end Gemini adapter smoke request; set `PRODEX_BIN` or `PRODEX_LIVE_GEMINI_MODEL` to override the binary or model. Add `PRODEX_LIVE_GEMINI_EXTENDED=1` for command-output-only, file edit, `apply_patch`, reference-repo clone/inspection, optional-tool update discipline, semantic compact, and explicit `exec resume` checks. Add `PRODEX_LIVE_GEMINI_MCP=1` and/or `PRODEX_LIVE_GEMINI_MULTIMODAL=1` when the local environment should also exercise MCP and image-input paths.

</details>

<details>
<summary>Presidio, memory, and optimizer internals (advanced)</summary>

Before launch, Super asks whether to add Presidio redaction. Empty input or `n` keeps Presidio disabled; answer `y` or pass `--presidio` to add the `presidio` prefix. Use `--no-presidio` to make the disabled choice explicit for non-interactive use. Super then asks whether to enable prodex-memory through managed Mem0 Docker. Empty input or `n` leaves prodex-memory disabled; answer `y` or pass `--mem0` to start the managed Mem0 OSS Docker server and route its OpenAI-compatible calls through Prodex gateway. Use `--no-mem0` to skip that prompt, or the `mem` prefix for local SQLite memory.

Prodex now supports multi-language Presidio redaction, including automatic detection and multi-language merging. The runtime uses `presidio.toml` endpoints and language configuration when available, falling back to `http://localhost:5002` and `http://localhost:5001` for Analyzer/Anonymizer URLs, and English (`en`) for language if not specified. It honors `fail_mode = "open"` or `"closed"`.

Example `presidio.toml` with multi-language support (English and Indonesian):

```toml
enabled = true
analyzer_url = "http://localhost:5002"
anonymizer_url = "http://localhost:5001"
language_mode = "auto"
languages = ["en", "id"]
fail_mode = "open"
```

Note that the default Microsoft Presidio Docker images typically only support English (`en`). To support other languages like Indonesian (`id`), you must use a custom Presidio Analyzer image with the necessary language models and recognizers installed. Prodex routing and configuration support the languages, but the actual detection quality depends on your Analyzer.

It keeps exact pass-through for continuation-sensitive requests. When safe, it uses adaptive token budgeting, artifact-backed large tool outputs, duplicate suppression, blob/noise detection, stable cache-friendly context framing, and critical-signal self-checks to reduce token load without dropping failure details.

The Super optimization stack is meant to stay deterministic and local by default. It auto-registers `sqz-mcp` and `token-savior` MCP servers when those binaries are already on `PATH` or in a managed `prodex-optimizers` checkout, exposes `sqz` and `claw-compactor` wrappers when discoverable, routes compatible optimizer cache/state under `PRODEX_HOME` instead of the workspace, and uses a dedicated runtime proxy for local compaction, stable references, and lower-token context shaping rather than hidden remote summarization.

RTK handles upstream/input command output before it enters the context window, using visible `rtk <cmd>` commands and overlay auto-wrappers when available. Auto-wrappers are only a backstop; write `rtk <cmd>` explicitly when you want the TUI/transcript to show RTK usage. SQZ handles downstream/context reuse after content is already in the session, using `prodex-sqz` when the MCP server is available.

Managed optimizer checkouts are discovered from `PRODEX_OPTIMIZERS_HOME`, `$XDG_DATA_HOME/prodex-optimizers`, then `~/.local/share/prodex-optimizers`.

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
prodex profile export
prodex profile remove second
prodex profile remove --all
```

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
prodex context compress ~/.codex/AGENTS.md --dry-run
git diff | prodex context compact-output --kind git-diff
```

| Command | Description |
|---|---|
| `prodex info` | Shows provider route/quota shapes plus effective runtime tuning values after environment, policy, and default resolution. |
| `prodex log` | Shows the latest session transcript text plus the latest runtime token event. |
| `prodex log stream` | Follows session/runtime logs and prints transcript text plus token events live. Add `--json` for JSON Lines token events only. |
| `prodex log upstream` | Follows backend-bound LLM payload snapshots after Prodex processing such as Presidio redaction and Smart Context rewriting. Add `--json` for JSON Lines payload events. Snapshots are capped at 128 KiB per payload. |
| `prodex doctor --install` | Adds install and embedded asset checks to doctor output. |
| `prodex doctor --runtime` | Runs runtime diagnostics. |
| `prodex doctor --bundle PATH --redacted` | Writes a shareable JSON diagnostic bundle without stored auth tokens or headers. |
| `prodex setup --dry-run` | Shows setup reconciliation actions without changing files. |
| `prodex capability list` | Lists built-in and optional Prodex capabilities with availability status. |
| `prodex context audit` | Reports approximate token weight for shared instruction and memory files. |
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
- [docs/testing.md](./docs/testing.md) — contributor testing guidance

## Support

If you find `prodex` useful and want to support its development, you can donate here:

[<img src="https://www.paypalobjects.com/en_US/i/btn/btn_donateCC_LG.gif" border="0" alt="Donate with PayPal" />](https://paypal.me/christiandoxa)
