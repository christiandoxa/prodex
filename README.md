# prodex

`prodex` is a multi-account, multi-provider Codex wrapper with auto-rotation.

Use multiple Codex accounts and supported provider backends from one command line. OpenAI/Codex profiles get quota-aware routing and auto-rotation; provider adapters let `prodex s` launch the Codex front end against Gemini, Anthropic, Copilot, DeepSeek, and local OpenAI-compatible servers.

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
- rotate to another account when quota runs out
- launch Codex/Super against non-OpenAI providers without changing front ends
- keep profile credentials separated
- keep sessions attached to the profile that created them
- run Codex, Caveman mode, Super mode, and Claude Code through the same wrapper

If you only use one Codex account and do not need quota rotation, you probably do not need `prodex`.

## Requirements

You need at least one logged-in Prodex profile.

| Tool | Used by |
|---|---|
| Codex CLI | `prodex`, `prodex run`, `prodex caveman`, `prodex super` |
| Claude Code | `prodex claude` |
| Claude-Mem | `mem` variants |
| RTK | `rtk` variants and `prodex s` / `prodex super` |

## Supported providers

Prodex supports two provider paths:

- **Profile-backed routing**: persisted profiles that Prodex can select, rotate, and inspect where provider APIs allow it.
- **Runtime provider launch**: `prodex s --provider ...` / `prodex super --provider ...` starts Codex with a temporary provider bridge for that session.

| Provider | Launch to Codex | Auth path | Quota view | Notes |
|---|---:|---|---:|---|
| OpenAI / Codex | `prodex`, `prodex run`, `prodex s` | ChatGPT OAuth, device code, or OpenAI/API-compatible key via `prodex login` | Yes | Full quota preflight and profile auto-rotation. |
| Google Gemini | `prodex s --provider gemini` | Google OAuth via `prodex login --with-google`, or `GEMINI_API_KEY` / `--api-key` | OAuth profiles | OAuth mode can rotate across Gemini profiles; API-key mode is runtime-only. |
| Anthropic Claude | `prodex s --provider anthropic` | Claude Code OAuth via `prodex login --with-claude` / `prodex profile import claude`, or `ANTHROPIC_API_KEY(S)` / `--api-key` | OAuth profiles | Shows Claude OAuth readiness; add `ANTHROPIC_ADMIN_KEY` to include Anthropic Admin rate-limit groups. |
| GitHub Copilot | `prodex s --provider copilot` | Imported Copilot CLI profile via `prodex profile import copilot`, or `GITHUB_COPILOT_API_KEY` / `--api-key` | Imported profiles | Native Copilot profile mode keeps continuations bound to the owning profile. |
| DeepSeek | `prodex s --provider deepseek` | `DEEPSEEK_API_KEY(S)` / `--api-key` | API-key balance | `prodex quota --all --provider deepseek` reads DeepSeek `/user/balance`. |
| Local OpenAI-compatible | `prodex super --url http://127.0.0.1:8131` | Local server auth/config | Health snapshot | `prodex quota --all --provider local --base-url ...` checks the local `/models` endpoint. |
| Bedrock / custom Codex `model_provider` | `prodex run` / `prodex caveman` direct pass-through | Codex-owned config | Config snapshot | Prodex reports configured provider metadata; provider-side quota stays owned by Codex/upstream. |

<details>
<summary>Provider behavior details</summary>

The auto-rotate proxy is intentionally conservative. It rotates only before a request or stream is committed, preserves `previous_response_id`, turn-state, and session affinity, and does not rotate mid-stream. OpenAI/Codex remains the default quota-aware pool. Gemini OAuth, imported Copilot profiles, Anthropic OAuth profiles, DeepSeek API keys, local OpenAI-compatible URLs, and Bedrock/custom Codex providers now have `prodex quota` views. Anthropic, DeepSeek, API-key Gemini, API-key Copilot, local URLs, and Bedrock/custom Codex providers still skip OpenAI quota preflight.

</details>

## Installation

<details>
<summary>Install from npm</summary>

```bash
npm install -g @christiandoxa/prodex
```

</details>

<details>
<summary>Install from source checkout</summary>

```bash
cargo install --path .
```

If you install from source, make sure the `codex` binary in your `PATH` is already installed and up to date.

</details>

## Optional tools

`prodex` can run without Claude-Mem, RTK, SQZ, token-savior, claw-compactor, or Presidio.

Install them only if you want to use commands such as:

<details>
<summary>Optional tool commands</summary>

```bash
prodex caveman mem
prodex caveman mem rtk
prodex rtk
prodex sqz
prodex tokensavior
prodex clawcompactor
prodex presidio doctor
prodex presidio redact --text "My phone is 212-555-1234"
prodex s
prodex super
prodex claude mem
prodex claude caveman mem
```

</details>

<details>
<summary>Install Claude-Mem</summary>

Claude-Mem is used by the `mem` variants.

Recommended install:

```bash
npx claude-mem install
```

Then follow the interactive prompts.

You can also install it from inside Claude Code:

```text
/plugin marketplace add thedotmack/claude-mem
/plugin install claude-mem
```

After installation, restart Claude Code or your coding CLI.

> Do not use `npm install -g claude-mem` as the main install method. That installs the SDK/library only; it does not register the plugin hooks or start the worker service.

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

Prodex auto-registers `prodex-sqz` for Super/Caveman overlay sessions when `sqz-mcp` is discoverable.

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

Fast Docker install using Microsoft's published images:

```bash
docker pull mcr.microsoft.com/presidio-analyzer
docker pull mcr.microsoft.com/presidio-anonymizer

docker run -d --name presidio-analyzer -p 5002:3000 mcr.microsoft.com/presidio-analyzer:latest
docker run -d --name presidio-anonymizer -p 5001:3000 mcr.microsoft.com/presidio-anonymizer:latest
```

Source checkout with Compose:

```bash
git clone https://github.com/microsoft/presidio.git ~/.local/share/presidio
cd ~/.local/share/presidio
docker compose -f docker-compose-text.yml up -d --build
```

Verify with Prodex:

```bash
prodex presidio doctor
prodex presidio redact --text "My name is John Smith and my phone is 212-555-1234."
prodex presidio enable
```

When you answer `y` to the `prodex super` / `prodex s` Presidio prompt or pass `--presidio`, Super starts a dedicated runtime proxy that redacts UTF-8 HTTP request bodies and WebSocket text frames through the local Presidio Analyzer and Anonymizer before forwarding them upstream. This is equivalent to adding the `presidio` prefix to the Super stack. Use `--no-presidio` to skip the prompt and keep redaction disabled. The runtime uses `presidio.toml` endpoints when configured, falls back to `http://localhost:5002` and `http://localhost:5001`, and honors `fail_mode = "open"` or `"closed"`.

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
```

Interactive `prodex login` now asks for the login method before starting a browser. Choose ChatGPT browser login, device-code login, API-key login, Google sign-in for Gemini, or Claude sign-in through Claude Code OAuth. For API-key profiles, you can also set an OpenAI-compatible backend URL:

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
<summary>Super mode overview</summary>

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
- Claude-Mem transcript watching
- RTK shell-command guidance
- full-access launch mode
- Smart Context Autopilot in the runtime proxy
- deterministic/local accommodation for `sqz`, `token-savior`, and `claw-compactor` low-token workflows

```bash
prodex s
prodex s exec "review this repo"
```

`prodex super` expands to:

```bash
prodex caveman mem rtk sqz tokensavior clawcompactor --full-access
```

Before launch, Super asks whether to add Presidio redaction. Empty input or `n` keeps the expansion above. If you answer `y`, it is equivalent to:

```bash
prodex caveman mem rtk sqz tokensavior clawcompactor presidio --full-access
```

Use `prodex super --presidio` to enable Presidio without prompting, or `prodex super --no-presidio` to skip the prompt and keep Presidio disabled. Presidio enables runtime request-body and WebSocket text redaction through local Presidio for the session. The runtime uses `presidio.toml` endpoints when configured, falls back to `http://localhost:5002` and `http://localhost:5001`, and honors `fail_mode = "open"` or `"closed"`.

Full access maps to Codex's sandbox-bypass launch flag. Use it only when you intentionally want Codex to run without the normal approval and sandbox protections.

Super's built-in optimization stack is deliberately local and deterministic. It preloads the existing Caveman and Claude-Mem pieces, exposes an overlay `rtk` wrapper plus RTK auto-wrappers for common noisy commands when RTK is installed, auto-registers `sqz-mcp` and `token-savior` MCP servers when those binaries are already on `PATH` or in a managed `prodex-optimizers` checkout, exposes `sqz` and `claw-compactor` wrappers when those commands/checkouts are discoverable, invokes a trusted one-shot `prodex-claw-compactor-sessionstart` SessionStart benchmark probe when Claw-Compactor is available, falls back to a temporary shadow `MEMORY.md` when the workspace has no Markdown memory files, then uses Smart Context Autopilot through a dedicated runtime proxy for lower-token request shaping. The probe delegates to `prodex-claw-compactor-auto "$(pwd)"` and uses a marker under `CODEX_HOME` so Codex conversation restarts do not replay it. Presidio redaction is added to that proxy only when you opt in at the prompt. Prodex passes token-savior cache and stats paths under `PRODEX_HOME` (default `~/.prodex`) so compatible token-savior versions keep generated state out of worktrees.

RTK and SQZ split the token work across different sides of the flow:

- RTK works upstream/input-side. Use visible `rtk <cmd>` for noisy terminal commands before their output enters the model context, such as `git diff`, `cargo test`, `npm test`, build logs, and package-manager output. Prodex also auto-wraps common noisy commands as a fallback when RTK is installed, but that fallback does not make the TUI show an `rtk` prefix.
- SQZ works downstream/context-side through the auto-registered `prodex-sqz` MCP server. Use it for repeated workspace reads, large text blobs, and long-session context reuse instead of emitting the same full content again.

Managed optimizer checkouts are discovered from `PRODEX_OPTIMIZERS_HOME`, `$XDG_DATA_HOME/prodex-optimizers`, then `~/.local/share/prodex-optimizers`.

</details>

## Commands

<details>
<summary>Most used commands</summary>

```bash
prodex
prodex s
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
```

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
```

The live `prodex quota --all --detail` view accepts `s` to cycle sort modes and `f` to cycle the provider filter through `all`, `openai`, `gemini`, `anthropic`, `copilot`, `deepseek`, and `local`. Add `--provider openai`, `--provider gemini`, `--provider anthropic`, `--provider copilot`, `--provider deepseek`, or `--provider local` to start locked to a single provider.

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

## Modes

| Mode | Command | Description |
|---|---|---|
| Normal Codex | `prodex` or `prodex run` | Managed Codex launch with profile selection and quota routing. |
| Caveman | `prodex caveman` | Runs Codex with a temporary overlay `CODEX_HOME`. |
| Super | `prodex s` or `prodex super` | Daily mode with Caveman, Claude-Mem, RTK guidance, full access, and deterministic/local token optimizations. |
| Claude Code | `prodex claude` | Runs Claude Code through Prodex-managed state. |

<details>
<summary>Normal Codex</summary>

```bash
prodex
prodex run
prodex run --profile main
prodex exec "review this repo"
```

</details>

<details>
<summary>Caveman mode</summary>

```bash
prodex caveman
prodex caveman mem
prodex caveman mem rtk
prodex rtk
prodex sqz
prodex tokensavior
prodex clawcompactor
prodex caveman --dry-run
prodex caveman --profile main
prodex caveman exec "review this repo in caveman mode"
prodex caveman 019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9
```

`prodex caveman` runs Codex with a temporary overlay `CODEX_HOME`, so the base profile home stays unchanged after the session ends.

If you use the `mem` variant, Prodex points an existing Claude-Mem Codex setup to the active Prodex session path instead of the default `~/.codex/sessions`.

Add optimizer prefixes before Codex args when you want Prodex to inject a specific launch overlay for that session: `mem`, `rtk`, `sqz`, `tokensavior`, `clawcompactor`, or `presidio`. Top-level shortcuts such as `prodex rtk` and `prodex sqz` map to `prodex caveman <prefix>`.

RTK is still an external binary. Install it separately if `rtk gain` is unavailable.

</details>

<details>
<summary>Super mode</summary>

```bash
prodex s
prodex s exec "review this repo"
ANTHROPIC_API_KEY=... prodex s --provider anthropic --model claude-sonnet-4-6
prodex profile import copilot
prodex s --provider copilot --model gpt-5.1-codex
DEEPSEEK_API_KEY=... prodex s --provider deepseek --model deepseek-v4-pro
prodex s --provider gemini
prodex super
prodex super --profile main
prodex super --dry-run
prodex super 019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9
```

`prodex s` is the short alias for `prodex super`.

This is my daily mode. It is the path I keep tuning for normal work: Caveman enabled, Claude-Mem transcript watching enabled, RTK guidance enabled, full access available, and context handling handled by the runtime proxy.

Super mode uses Prodex's slim Claude-Mem Codex schema by default to avoid storing full assistant/tool output in recall context.

Use `--mem-super-slim` to store prompt summaries/references instead of full prompt bodies:

```bash
prodex super --mem-super-slim
```

Use `--mem-full` when you need the full transcript schema:

```bash
prodex super --mem-full
```

Super also enables Smart Context Autopilot in the runtime proxy.

Use `--provider anthropic` when you want the Codex/Super front end with Anthropic upstream:

```bash
prodex login --with-claude
prodex s --provider anthropic --model claude-sonnet-4-6
prodex s --provider anthropic --model claude-sonnet-4-6 --api-key "$ANTHROPIC_API_KEY"
```

If `--api-key` is omitted, Prodex uses the Anthropic profile created by `prodex login --with-claude` or `prodex profile import claude`. API-key mode still reads `ANTHROPIC_API_KEY`; `ANTHROPIC_API_KEYS` may contain multiple comma-, semicolon-, or newline-separated keys for round-robin request rotation. This path injects a temporary `prodex-anthropic` Codex provider, exposes a local `/v1/responses` adapter to Codex, forwards to Anthropic's OpenAI-compatible chat API, and keeps quota preflight disabled. `prodex quota --all --provider anthropic` shows OAuth readiness for Anthropic profiles; set `ANTHROPIC_ADMIN_KEY` when you want Anthropic Admin rate-limit groups included.

Use `--provider copilot` when you want the Codex/Super front end with GitHub Copilot upstream:

```bash
prodex profile import copilot
prodex s --provider copilot --model gpt-5.1-codex
```

Without `--api-key`, Prodex uses imported Copilot CLI profiles, refreshes Copilot runtime API tokens from GitHub before launch, rotates fresh native Responses requests across eligible profiles, and binds streaming response IDs back to the owning profile for continuations. `GITHUB_COPILOT_API_KEY` or `--api-key` can be used when you already have a Copilot runtime API token.

Use `--provider deepseek` when you want the Codex/Super front end with DeepSeek as the upstream model:

```bash
prodex s --provider deepseek --model deepseek-v4-pro --api-key "$DEEPSEEK_API_KEY"
```

If `--api-key` is omitted, Prodex reads `DEEPSEEK_API_KEY`; `DEEPSEEK_API_KEYS` may contain multiple comma-, semicolon-, or newline-separated keys for round-robin request rotation. This path injects a temporary `prodex-deepseek` Codex provider, exposes a local `/v1/responses` adapter to Codex, forwards to DeepSeek's OpenAI-format chat API, and keeps quota preflight disabled. Prodex also injects a one-model Codex catalog for the selected DeepSeek model, so `/model` stays on that model and offers the DeepSeek-compatible `high`/`xhigh` effort choices. `prodex quota --all --provider deepseek` reads the same `DEEPSEEK_API_KEY(S)` environment and fetches DeepSeek `/user/balance`. The Super optional tools still run normally because they are local launch overlays around Codex. Remote compact is not implemented for this adapter yet, so the default DeepSeek context window is large and `--auto-compact-token-limit` defaults high.

Use `--provider gemini` when you want the Codex/Super front end with Gemini upstream:

```bash
prodex login --with-google
prodex s --provider gemini
GEMINI_API_KEY=... prodex s --provider gemini --model gemini-2.5-pro
prodex s --provider gemini --model gemini-2.5-pro --api-key "$GEMINI_API_KEY"
```

Without `--api-key`, Prodex uses the Google OAuth profile created by `prodex login` and routes through Google's Code Assist Gemini endpoint. With `--api-key`, or `GEMINI_API_KEY` / `GOOGLE_API_KEY`, Prodex routes through the public Gemini API. The default model is `gemini-2.5-pro`, matching Gemini CLI's current default, and the injected catalog exposes Gemini reasoning efforts with the 2.5 default thinking budget of 8192. `prodex quota` reads the same Google OAuth profile and fetches Gemini Code Assist `retrieveUserQuota` bucket data. The Super optional tools still run as local Codex overlays on this path: Prodex maps Codex/MCP tool schemas to Gemini function declarations, honors `auto` / `none` / `required` tool-choice modes, and preserves Gemini thought signatures across tool-call followups.

Before launch, Super asks whether to add Presidio redaction. Empty input or `n` keeps Presidio disabled; answer `y` or pass `--presidio` to add the `presidio` prefix. Use `--no-presidio` to make the disabled choice explicit for non-interactive use.

It keeps exact pass-through for continuation-sensitive requests. When safe, it uses adaptive token budgeting, artifact-backed large tool outputs, duplicate suppression, blob/noise detection, stable cache-friendly context framing, and critical-signal self-checks to reduce token load without dropping failure details.

The Super optimization stack is meant to stay deterministic and local by default. It auto-registers `sqz-mcp` and `token-savior` MCP servers when those binaries are already on `PATH` or in a managed `prodex-optimizers` checkout, exposes `sqz` and `claw-compactor` wrappers when discoverable, routes compatible token-savior cache/state under `PRODEX_HOME` instead of the workspace, and uses a dedicated runtime proxy for local compaction, stable references, and lower-token context shaping rather than hidden remote summarization.

RTK handles upstream/input command output before it enters the context window, using visible `rtk <cmd>` commands and overlay auto-wrappers when available. Auto-wrappers are only a backstop; write `rtk <cmd>` explicitly when you want the TUI/transcript to show RTK usage. SQZ handles downstream/context reuse after content is already in the session, using `prodex-sqz` when the MCP server is available.

Managed optimizer checkouts are discovered from `PRODEX_OPTIMIZERS_HOME`, `$XDG_DATA_HOME/prodex-optimizers`, then `~/.local/share/prodex-optimizers`.

</details>

<details>
<summary>Claude Code</summary>

```bash
prodex claude -- -p "summarize this repo"
prodex claude mem -- -p "recall past work on this repo"
prodex claude caveman
prodex claude caveman mem
prodex claude caveman -- -p "summarize this repo briefly"
prodex claude caveman mem -- -p "summarize this repo briefly"
prodex claude --profile second caveman -- -p "review the latest diff briefly"
prodex claude --profile second -- -p --output-format json "show the latest diff"
```

`prodex claude` uses the normal Claude Code flow while keeping state under Prodex-managed configuration.

`prodex claude caveman` enables Caveman for that session while keeping state under the Prodex-managed `CLAUDE_CONFIG_DIR`, not the global `~/.claude`.

`prodex claude caveman mem` combines Caveman and Claude-Mem.

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
| `prodex info` | Shows effective runtime tuning values after environment, policy, and default resolution. |
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

In practice, profile `history.jsonl`, `sessions`, `config.toml`, `environments.toml`, plugins, skills, app-server plugin state, memory-extension state, and Codex runtime SQLite files such as `state_*`, `goals_*`, `logs_*`, and `memories_*` link to the same Codex home that direct Codex uses.

Prodex does not synthesize legacy Codex `[profiles.*]` behavior. File-based Codex profile config selected by `--profile` stays in shared Codex state, while Prodex-owned account selection remains in Prodex profile metadata.

Prodex also leaves packaged Codex runtime resources alone, including Codex 0.135.0's bundled patched zsh helper under the Codex package layout. Do not set `zsh_path` through Prodex unless you are intentionally debugging direct Codex config.

This matches direct Codex behavior: logging out or switching accounts does not hide chat history.

Older Prodex state from `$PRODEX_HOME/.codex` is merged into the native Codex home on the next managed-profile launch.

Set `PRODEX_SHARED_CODEX_HOME` only when you intentionally want a different shared Codex root.

</details>

<details>
<summary>Bedrock and custom providers</summary>

Auto-rotate and quota checks apply to supported OpenAI/Codex profiles. `prodex quota` also supports Google Gemini OAuth profiles, Anthropic OAuth profiles, imported Copilot accounts, DeepSeek API-key balances, local OpenAI-compatible health snapshots, and configured custom providers.

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
- [docs/testing.md](./docs/testing.md) — contributor testing guidance

## Support

If you find `prodex` useful and want to support its development, you can donate here:

[<img src="https://www.paypalobjects.com/en_US/i/btn/btn_donateCC_LG.gif" border="0" alt="Donate with PayPal" />](https://paypal.me/christiandoxa)
