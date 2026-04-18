# prodex

[![CI](https://github.com/christiandoxa/prodex/actions/workflows/ci.yml/badge.svg)](https://github.com/christiandoxa/prodex/actions/workflows/ci.yml)

`prodex` is a wrapper around Codex / Claude Code setups when you want to work with multiple isolated profiles.

Main use case: one account per profile, quota checked before launch, and new sessions can hop to another ready profile when the current one is not a good candidate. Existing sessions stay pinned to the profile they started with.

It can also launch Caveman mode, and optionally wire Claude-Mem into the selected session path.

## Features

- one account = one profile
- isolated profile homes
- quota preflight before launch
- rotates fresh work to another ready profile
- keeps continuation/session affinity
- `prodex caveman` for Codex + Caveman
- `prodex caveman mem` for Codex + Caveman + Claude-Mem
- `prodex claude` for Claude Code through the same pool
- `prodex claude caveman` for Claude Code + Caveman
- `prodex claude caveman mem` for Claude Code + Caveman + Claude-Mem

## Requirements

You need at least one logged-in Prodex profile.

Depending on what you want to run:

- Codex CLI for `prodex` and `prodex caveman`
- Claude Code (`claude`) for `prodex claude` and `prodex claude caveman`
- optionally `claude-mem` for `mem` variants

Installing `@christiandoxa/prodex` from npm also installs the pinned Codex runtime dependency:

```bash
npm install -g @christiandoxa/prodex
````

That pulls in `@openai/codex@0.121.0` as well. Claude Code is still separate.

If you want Claude-Mem support, install it with the upstream installer:

```bash
npx claude-mem install --ide codex-cli
npx claude-mem install --ide claude-code
npx claude-mem start
```

## Install

### npm

```bash
npm install -g @christiandoxa/prodex
```

### Cargo

```bash
cargo install prodex
```

If you want a pinned version:

```bash
npm install -g @christiandoxa/prodex@0.24.0
cargo install prodex --force --version 0.24.0
```

## Quick start

If your current shared Codex home is already logged in:

```bash
prodex profile import-current main
```

Or do it from scratch:

```bash
prodex login
prodex profile add second
prodex login --profile second
```

Import a currently logged-in Copilot CLI account into Prodex metadata:

```bash
prodex profile import copilot
prodex profile import copilot --name copilot-main --activate
```

Check what Prodex sees:

```bash
prodex profile list
prodex quota --all
```

Run stuff through Prodex:

```bash
prodex
prodex caveman
prodex caveman mem
prodex exec "review this repo"
prodex claude -- -p "summarize this repo"
prodex claude mem -- -p "recall past work on this repo"
prodex claude caveman -- -p "summarize this repo briefly"
prodex claude caveman mem -- -p "summarize this repo briefly"
```

`prodex` with no subcommand is just `prodex run`.

## Commands

### Profiles

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

A note on Copilot import:

`prodex profile import copilot` does **not** move the Copilot token into Prodex-managed storage. The token stays where Copilot already keeps it. Prodex only records the provider identity and API endpoint in its own metadata.

The imported profile shows up in the pool and export/import flows, but `prodex run`, `prodex login`, `prodex logout`, and `prodex quota` still only work with OpenAI/Codex profiles right now.

### Codex

```bash
prodex
prodex run
prodex run --profile main
prodex exec "review this repo"
prodex run 019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9
printf 'context from stdin' | prodex run exec "summarize this"
```

### Caveman

```bash
prodex caveman
prodex caveman mem
prodex caveman --profile main
prodex caveman exec "review this repo in caveman mode"
prodex caveman 019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9
```

`prodex caveman` uses the Caveman plugin from [JuliusBrussee/caveman](https://github.com/JuliusBrussee/caveman) and launches Codex with a temporary overlay `CODEX_HOME`.

That matters because the base profile home is left alone after the session exits.

If you use the `mem` variant, Prodex points an existing Claude-Mem Codex setup at the active Prodex session path instead of the default `~/.codex/sessions` tree.

### Claude Code

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

Use `prodex claude` for the normal Claude Code path.

Use `prodex claude caveman` if you want Claude Code with Caveman preloaded.

The `caveman` prefix loads the plugin for that session only while still keeping state under the Prodex-managed `CLAUDE_CONFIG_DIR`, not global `~/.claude`.

The `mem` prefix loads an existing Claude-Mem Claude Code install via Claude's `--plugin-dir` support.

`prodex claude caveman mem` combines both.

### Export / quota / debug

```bash
prodex profile export
prodex quota --all
prodex quota --all --once
prodex doctor --runtime
```

## Notes

This is basically a profile/session router for people who do a lot of CLI-driven agent work and do not want everything tied to one mutable global home.

If you only use one account and do not care about quota-aware routing or keeping sessions attached to their original profile, you probably do not need it.

## More

See [QUICKSTART.md](./QUICKSTART.md) for the longer walkthrough.
