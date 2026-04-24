# prodex

`prodex` is a wrapper for Codex and Claude Code for working with multiple profiles.

The main feature is auto rotate. If one OpenAI/Codex profile runs out of quota, `prodex` can route new work to another profile that is still available. You do not need to switch accounts manually.

It also keeps profiles isolated and keeps existing sessions attached to the profile they started with. For Codex CLI 0.124.0 and newer, Codex itself supports Amazon Bedrock and other OpenAI-compatible custom providers through `model_provider`; Prodex passes those profiles through directly instead of adding its OpenAI quota and rotation layer.

## Why use it

Use `prodex` if you want to:

- automatically use another available profile when quota runs out
- work with multiple accounts
- keep each profile isolated
- keep sessions tied to the original profile

If you only use one account and do not need profile isolation or quota-aware routing, you probably do not need it.

## Requirements

You need at least one logged-in Prodex profile.

Depending on your setup, you may also need:

- Codex CLI for `prodex` and `prodex caveman`
- Claude Code for `prodex claude`
- `claude-mem` for `mem` variants

## Install

### npm

```bash
npm install -g @christiandoxa/prodex
````

### Cargo

```bash
cargo install prodex
```

If you install with Cargo, make sure the `codex` binary in your `PATH` is already installed and up to date.

## Quick start

If your current Codex home is already logged in:

```bash
prodex profile import-current main
```

Or set it up from scratch:

```bash
prodex login
prodex profile add second
prodex login --profile second
```

You can also import a logged-in Copilot CLI account:

```bash
prodex profile import copilot
prodex profile import copilot --name copilot-main --activate
```

Check your profiles and quota:

```bash
prodex profile list
prodex quota --all
```

Run through `prodex`:

```bash
prodex
prodex caveman
prodex caveman mem
prodex super
prodex exec "review this repo"
prodex claude -- -p "summarize this repo"
prodex claude mem -- -p "recall past work on this repo"
```

## Common commands

### Run Codex

```bash
prodex
prodex run
prodex run --profile main
prodex exec "review this repo"
prodex run 019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9
printf 'context from stdin' | prodex run exec "summarize this"
```

### Run Caveman mode

```bash
prodex caveman
prodex caveman mem
prodex caveman --profile main
prodex caveman exec "review this repo in caveman mode"
prodex caveman 019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9
```

`prodex caveman` runs Codex with a temporary overlay `CODEX_HOME`, so the base profile home stays unchanged after the session ends.

If you use the `mem` variant, Prodex points an existing Claude-Mem Codex setup to the active Prodex session path instead of the default `~/.codex/sessions`.

### Run Super mode

```bash
prodex super
prodex super --url http://127.0.0.1:8131
prodex super --profile main
prodex super exec "review this repo in super mode"
prodex super 019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9
```

`prodex super` is a shortcut for `prodex caveman mem --full-access`.

Use this when you want Caveman mode, Claude-Mem transcript watching, and launch-time full access together. Full access maps to Codex's sandbox-bypass launch flag, so use it only when you intentionally want Codex to run without the normal approval and sandbox protections.

Use `prodex super --url http://127.0.0.1:8131` when you want the same Super mode front end to talk directly to a local OpenAI-compatible server such as `llama-server`. Prodex injects a temporary `prodex-local` Codex provider, appends `/v1` when the URL has no path, disables non-function native tools that local servers commonly reject, advertises a conservative 16k local context window, and skips quota/proxy routing for that launch. The default local model id is `unsloth/qwen3.5-35b-a3b`; override it with `--model`, for example `prodex super --url http://127.0.0.1:8131 --model local/qwen`. Use `--context-window` and `--auto-compact-token-limit` if your local server is configured larger. See [LOCAL.md](./LOCAL.md) for self-hosted model setup and testing.

### Run Claude Code

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

`prodex claude` uses the normal Claude Code flow.

`prodex claude caveman` enables Caveman for that session while keeping state under the Prodex-managed `CLAUDE_CONFIG_DIR`, not the global `~/.claude`.

`prodex claude caveman mem` combines Caveman and Claude-Mem.

## Profile commands

```bash
prodex profile list
prodex profile add second
prodex profile import copilot
prodex profile import-current main
prodex login --profile second
prodex use --profile main
prodex logout --profile main
prodex profile remove second
prodex profile remove --all
```

## Bedrock and custom providers

Auto rotate and quota checks apply to supported OpenAI/Codex profiles.

Codex CLI 0.124.0 added first-class Amazon Bedrock and OpenAI-compatible custom provider support. Configure that in the selected profile's Codex `config.toml`, for example with `model_provider = "amazon-bedrock"`.

If a profile's `config.toml` sets `model_provider` to a non-OpenAI backend such as `amazon-bedrock`, `prodex run` and `prodex caveman` launch Codex directly without quota preflight or the local auto-rotate proxy. Bedrock quota, credentials, regions, and provider errors are handled by Codex and the upstream provider, not by Prodex.

`prodex quota` is not available for those profiles.

`prodex claude` is only supported with the default OpenAI/Codex provider.

When you import a Copilot profile, Prodex does not move the Copilot token into Prodex-managed storage. It only records the provider identity and API endpoint in its own metadata.

## Utility commands

```bash
prodex profile export
prodex quota --all
prodex quota --all --once
prodex doctor --runtime
```

## More

See [QUICKSTART.md](./QUICKSTART.md) for a longer walkthrough, and [LOCAL.md](./LOCAL.md) for self-hosted local model setup.

Contributor testing guidance lives in [docs/testing.md](./docs/testing.md), including the fast/serial split and runtime parallel-safety assumptions.
