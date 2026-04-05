# Quick Start

One OpenAI profile pool. Two CLIs.

Use `prodex` for Codex CLI. Use `prodex claude` for Claude Code. Both commands run on top of the same OpenAI-backed Prodex profile pool.

## Requirements

- An OpenAI account, plus at least one logged-in Prodex profile
- Codex CLI if you want to use `prodex`
- Claude Code (`claude`) if you want to use `prodex claude`

If you install `@christiandoxa/prodex` from npm, the Codex runtime dependency is installed for you. Claude Code is still a separate CLI and should already be installed when you use `prodex claude`.

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

The current local version in this repo is `0.2.114`:

```bash
npm install -g @christiandoxa/prodex@0.2.114
cargo install prodex --force --version 0.2.114
```

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
prodex quota --all
prodex info
```

`prodex quota --all` refreshes live by default. Use `--once` when you want a single snapshot:

```bash
prodex quota --all --once
```

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

## 4. Run Claude Code with `prodex claude`

```bash
prodex claude -- -p "summarize this repo"
prodex claude --profile second -- -p --output-format json "show the latest diff"
```

Use this path when you want Claude Code to be the front end while Prodex still routes requests through your OpenAI-backed profile pool.

What changes on this path:

- Claude Code talks to a local Anthropic-compatible Prodex proxy
- each profile gets its own `CLAUDE_CONFIG_DIR`
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

## 5. Switch profiles explicitly

```bash
prodex use --profile main
prodex current
```

## 6. Debug the runtime

```bash
prodex doctor
prodex doctor --quota
prodex doctor --runtime
```

If a runtime session looks stalled, inspect the latest proxy log:

```bash
prodex doctor --runtime
tail -n 200 "$(cat /tmp/prodex-runtime-latest.path)"
```

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
