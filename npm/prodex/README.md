# prodex

Safe multi-account auto-rotate for Codex CLI and Claude Code.

`prodex` is a multi-account wrapper for Codex CLI and a profile-aware proxy launcher for Claude Code. It keeps per-profile state isolated, runs built-in quota checks, and rotates to another ready account before a request or stream is committed.

## Why prodex

- use the same Prodex profile pool from both `codex` and Claude Code
- auto-rotate to another ready account when the current one is quota-blocked or temporarily unhealthy
- keep each account isolated in its own `CODEX_HOME`, while `prodex claude` also keeps Claude Code state isolated per profile
- preserve continuation affinity so ongoing chains stay on the right account
- keep transport behavior close to the direct upstream CLIs

## Install

Install from npm:

```bash
npm install -g @christiandoxa/prodex
```

This is usually the lightest option because it does not need a local Rust build.

Or install from [crates.io](https://crates.io/crates/prodex):

```bash
cargo install prodex
```

The npm package version is kept in lockstep with the published crate version.

## Update Tips

Check your installed version first:

```bash
prodex --version
```

The current local binary version in this repo is `0.2.107`, so matching update commands look like this:

```bash
npm install -g @christiandoxa/prodex@0.2.107
cargo install prodex --force --version 0.2.107
```

If you just want the lighter install path, prefer npm over `cargo install` because npm does not need to compile `prodex` locally.

If you want to move from a Cargo-installed binary to npm, uninstall the Cargo binary first and then install the npm package:

```bash
cargo uninstall prodex
npm install -g @christiandoxa/prodex
```

## Quick Start

Import your current `codex` login:

```bash
prodex profile import-current main
```

Or create a profile through `codex login`:

```bash
prodex login
```

If you need device-code auth, pass it through unchanged:

```bash
prodex login --device-auth
```

Check quotas:

```bash
prodex quota --all
prodex info
```

Run Codex CLI with safe auto-rotate:

```bash
prodex
prodex run
```

Running `prodex` without a subcommand is shorthand for `prodex run`.

Resume a saved Codex session directly:

```bash
prodex run 019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9
```

Pin a specific profile when needed:

```bash
prodex run --profile second
```

`prodex run exec` also preserves stdin passthrough, so prompt and piped input stay together:

```bash
printf 'context from stdin' | prodex run exec "summarize this"
```

Run Claude Code CLI through the same profile pool:

```bash
prodex claude -- -p "summarize this repo"
```

## Core Commands

```bash
prodex
prodex exec "review this repo"
prodex profile list
prodex use --profile main
prodex info
prodex quota --all
prodex quota --all --once
prodex doctor
prodex doctor --runtime
prodex run
prodex run 019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9
prodex claude -- -p "summarize this repo"
```

## Notes

- `prodex` is not only for `codex`: `prodex run` targets Codex CLI, and `prodex claude` launches Claude Code through the same profile pool
- one `prodex` profile = one isolated `CODEX_HOME`, and `prodex claude` also gets an isolated per-profile `CLAUDE_CONFIG_DIR`
- `prodex login` still uses the real `codex login` flow, including `--device-auth`
- `prodex login` without `--profile` first tries to read the ChatGPT account email from `tokens.id_token` in `auth.json`, then falls back to the usage endpoint email when needed
- `prodex` without a subcommand behaves like `prodex run`
- `prodex run <session-id>` forwards to `codex resume <session-id>`
- `prodex info` summarizes profile count, the installed prodex version and update status, running Prodex processes, aggregated quota pool, and a no-reset runway estimate from active runtime logs
- `prodex quota` live-refreshes every 5 seconds by default, and `prodex quota --all` also shows aggregated `5h` and `weekly` pool remaining before the per-profile table
- Prodex-owned screens adapt to terminal width, and live views can also adapt to terminal height
- `prodex claude` injects a local Anthropic-compatible proxy for Claude Code and still uses Prodex profile selection and runtime rotation
- `prodex claude` also keeps Claude Code state in an isolated per-profile `CLAUDE_CONFIG_DIR`, so theme/login onboarding does not reappear every launch and the current workspace can open directly in chat mode
- `prodex claude` defaults to the current Codex model from shared `config.toml` when available, instead of always starting from plain `gpt-5`
- use `PRODEX_CLAUDE_BIN` if `claude` is not on `PATH`
- use `PRODEX_CLAUDE_MODEL` to override the upstream Responses model used for Anthropic-compatible requests
- use `PRODEX_CLAUDE_REASONING_EFFORT` to force the upstream Responses reasoning tier; for example, `PRODEX_CLAUDE_MODEL=gpt-5.2 PRODEX_CLAUDE_REASONING_EFFORT=xhigh prodex claude -- -p "hello"`

For a slightly longer setup guide, see [QUICKSTART.md](https://github.com/christiandoxa/prodex/blob/main/QUICKSTART.md).
