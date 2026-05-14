# prodex

`prodex` is a multi-account Codex wrapper with auto-rotation.

Use multiple Codex accounts from one command line. When the active account runs out of quota, `prodex` can route the next work to another available account.

## Contents

- [Why prodex](#why-prodex)
- [Requirements](#requirements)
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

## Installation

### npm

```bash
npm install -g @christiandoxa/prodex
```

### Source checkout

```bash
cargo install --path .
```

If you install from source, make sure the `codex` binary in your `PATH` is already installed and up to date.

## Optional tools

`prodex` can run without Claude-Mem or RTK.

Install them only if you want to use commands such as:

```bash
prodex caveman mem
prodex caveman mem rtk
prodex s
prodex super
prodex claude mem
prodex claude caveman mem
```

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
brew install rtk
```

### Linux/macOS quick install

```bash
curl -fsSL https://raw.githubusercontent.com/rtk-ai/rtk/refs/heads/master/install.sh | sh
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
cargo install --git https://github.com/rtk-ai/rtk
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
cargo install --git https://github.com/rtk-ai/rtk
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

## Quick start

### Import your current Codex login

If your current Codex home is already logged in:

```bash
prodex profile import-current main
```

### Or create profiles from scratch

```bash
prodex login
prodex profile add second
prodex login --profile second
```

### Check profiles and quota

```bash
prodex profile list
prodex quota --all
prodex session list
```

### Start Codex through Prodex

```bash
prodex
```

Or run a one-off prompt:

```bash
prodex exec "review this repo"
```

<details>
<summary>Import a Copilot CLI account</summary>

```bash
prodex profile import copilot
prodex profile import copilot --name copilot-main --activate
```

When you import a Copilot profile, Prodex does not move the Copilot token into Prodex-managed storage. It only records the provider identity and API endpoint in its own metadata.

</details>

## Daily command: `prodex s`

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

```bash
prodex s
prodex s exec "review this repo"
```

`prodex super` expands to:

```bash
prodex caveman mem rtk --full-access
```

Full access maps to Codex's sandbox-bypass launch flag. Use it only when you intentionally want Codex to run without the normal approval and sandbox protections.

## Commands

<details open>
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
```

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
| Super | `prodex s` or `prodex super` | Daily mode with Caveman, memory, RTK guidance, and full access. |
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
prodex caveman --dry-run
prodex caveman --profile main
prodex caveman exec "review this repo in caveman mode"
prodex caveman 019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9
```

`prodex caveman` runs Codex with a temporary overlay `CODEX_HOME`, so the base profile home stays unchanged after the session ends.

If you use the `mem` variant, Prodex points an existing Claude-Mem Codex setup to the active Prodex session path instead of the default `~/.codex/sessions`.

Add `rtk` after `mem` when you want Prodex to inject RTK shell-command guidance into the temporary Codex overlay for that launch.

RTK is still an external binary. Install it separately if `rtk gain` is unavailable.

</details>

<details>
<summary>Super mode</summary>

```bash
prodex s
prodex s exec "review this repo"
prodex super
prodex super --profile main
prodex super --dry-run
prodex super 019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9
```

`prodex s` is the short alias for `prodex super`.

This is my daily mode. It is the path I keep tuning for normal work: memory enabled, RTK guidance enabled, full access available, and context handling handled by the runtime proxy.

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

It keeps exact pass-through for continuation-sensitive requests. When safe, it uses adaptive token budgeting, artifact-backed large tool outputs, duplicate suppression, blob/noise detection, stable cacheable context, and critical-signal self-checks to reduce token load without dropping failure details.

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

<details open>
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
prodex doctor --runtime
prodex doctor --bundle ./prodex-doctor.json --redacted
prodex context audit
prodex context compress ~/.codex/AGENTS.md --dry-run
git diff | prodex context compact-output --kind git-diff
```

| Command | Description |
|---|---|
| `prodex info` | Shows effective runtime tuning values after environment, policy, and default resolution. |
| `prodex doctor --runtime` | Runs runtime diagnostics. |
| `prodex doctor --bundle PATH --redacted` | Writes a shareable JSON diagnostic bundle without stored auth tokens or headers. |
| `prodex context audit` | Reports approximate token weight for shared instruction and memory files. |
| `prodex context compress` | Compresses Markdown/text context files and writes an `.original.md` backup. |
| `prodex context compact-output` | Compacts copied command output such as `git status`, `git diff`, `rg`, `grep`, `find`, `tree`, or long logs. |

For full policy keys, environment overrides, and runtime log path resolution, see [docs/runtime-policy.md](./docs/runtime-policy.md).

</details>

## Advanced behavior

<details>
<summary>Shared Codex history</summary>

Managed Prodex profiles keep account credentials isolated per profile, but Codex-owned shared state uses the native Codex home by default.

On Unix-like systems, this is usually:

```bash
~/.codex
```

In practice, profile `history.jsonl`, `sessions`, `config.toml`, `environments.toml`, plugins, skills, and related shared files link to the same Codex home that direct Codex uses.

This matches direct Codex behavior: logging out or switching accounts does not hide chat history.

Older Prodex state from `$PRODEX_HOME/.codex` is merged into the native Codex home on the next managed-profile launch.

Set `PRODEX_SHARED_CODEX_HOME` only when you intentionally want a different shared Codex root.

</details>

<details>
<summary>Bedrock and custom providers</summary>

Auto-rotate and quota checks apply to supported OpenAI/Codex profiles.

If a profile's `config.toml` sets `model_provider` to a non-OpenAI backend such as `amazon-bedrock`, `prodex run` and `prodex caveman` launch Codex directly without quota preflight or the local auto-rotate proxy.

Bedrock quota, credentials, regions, and provider errors are handled by Codex and the upstream provider, not by Prodex.

`prodex quota` is not available for those profiles.

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
- [docs/runtime-policy.md](./docs/runtime-policy.md) — runtime policy keys, environment overrides, and runtime log path resolution
- [docs/testing.md](./docs/testing.md) — contributor testing guidance

## Support

If you find `prodex` useful and want to support its development, you can donate here:

[<img src="https://www.paypalobjects.com/en_US/i/btn/btn_donateCC_LG.gif" border="0" alt="Donate with PayPal" />](https://paypal.me/christiandoxa)
