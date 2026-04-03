# prodex

[![CI](https://github.com/christiandoxa/prodex/actions/workflows/ci.yml/badge.svg)](https://github.com/christiandoxa/prodex/actions/workflows/ci.yml)

One OpenAI profile pool for Codex CLI and Claude Code.

`prodex` gives you two entry points backed by the same OpenAI account pool:

| Use case | Command |
| --- | --- |
| Run Codex CLI through Prodex | `prodex` or `prodex run` |
| Run Claude Code through Prodex | `prodex claude` |

It keeps each profile isolated, checks quota before launch, and rotates to another ready account before a request or stream is committed.

Use `prodex` when Codex CLI is your front end. Use `prodex claude` when Claude Code is your front end. The account pool, profile isolation, quota checks, and continuation routing stay in Prodex either way.

## Requirements

- An OpenAI account, plus at least one logged-in Prodex profile
- Codex CLI if you want to use `prodex`
- Claude Code (`claude`) if you want to use `prodex claude`

If you install `@christiandoxa/prodex` from npm, the Codex runtime dependency is installed for you. Claude Code is still a separate CLI and should already be available on your `PATH` when you use `prodex claude`.

## Install

Install from npm:

```bash
npm install -g @christiandoxa/prodex
```

Or install from [crates.io](https://crates.io/crates/prodex):

```bash
cargo install prodex
```

The npm package version is kept in lockstep with the published crate version.

## Update

Check your installed version:

```bash
prodex --version
```

The current local version in this repo is `0.2.109`:

```bash
npm install -g @christiandoxa/prodex@0.2.109
cargo install prodex --force --version 0.2.109
```

If you want to switch from a Cargo-installed binary to npm:

```bash
cargo uninstall prodex
npm install -g @christiandoxa/prodex
```

## Start

Import your current login:

```bash
prodex profile import-current main
```

Or create a profile through the normal Codex login flow:

```bash
prodex login
prodex login --device-auth
```

Check the pool:

```bash
prodex profile list
prodex quota --all
prodex info
```

## Use `prodex` for Codex CLI

`prodex` without a subcommand is shorthand for `prodex run`.

```bash
prodex
prodex run --profile second
prodex run 019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9
printf 'context from stdin' | prodex run exec "summarize this"
```

Use this path when you want Codex CLI itself to be the front end. Prodex handles profile selection, quota preflight, continuation affinity, and safe pre-commit rotation across your OpenAI-backed profiles.

## Use `prodex claude` for Claude Code

```bash
prodex claude -- -p "summarize this repo"
prodex claude --profile second -- -p --output-format json "show the latest diff"
```

Use this path when you want Claude Code to be the front end while Prodex still routes requests through the same OpenAI-backed profile pool.

- `prodex claude` runs Claude Code through a local Anthropic-compatible proxy
- Claude Code state is isolated per profile in `CLAUDE_CONFIG_DIR`
- the initial Claude model follows the shared Codex `config.toml` model when available
- Claude's `opus`, `sonnet`, and `haiku` picker entries are pinned to representative GPT models
- Claude `max` effort maps to OpenAI `xhigh` when the selected GPT model supports it
- Claude Code itself only exposes built-in aliases plus one custom model option on third-party providers
- use `PRODEX_CLAUDE_BIN` if `claude` is not on `PATH`
- use `PRODEX_CLAUDE_MODEL` to force a specific upstream Responses model
- use `PRODEX_CLAUDE_REASONING_EFFORT` to force the upstream reasoning tier

Example:

```bash
PRODEX_CLAUDE_MODEL=gpt-5.2 PRODEX_CLAUDE_REASONING_EFFORT=xhigh prodex claude -- -p "hello"
```

## Common Commands

```bash
prodex profile list
prodex use --profile main
prodex current
prodex quota --all
prodex quota --all --once
prodex info
prodex doctor
prodex doctor --runtime
```

## More

For a slightly longer walkthrough, see [QUICKSTART.md](https://github.com/christiandoxa/prodex/blob/main/QUICKSTART.md).
