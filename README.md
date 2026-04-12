# prodex

[![CI](https://github.com/christiandoxa/prodex/actions/workflows/ci.yml/badge.svg)](https://github.com/christiandoxa/prodex/actions/workflows/ci.yml)

Run Codex, Claude Code, and their Caveman-mode variants on top of one OpenAI profile pool.

`prodex` manages isolated `CODEX_HOME` profiles, checks quota before launch, rotates fresh work to another ready profile when needed, and keeps existing continuations attached to the profile that already owns them.

## Highlights

- One account = one profile
- Built-in quota preflight and fresh-request rotation
- Continuation affinity for existing Codex sessions
- `prodex caveman` launches Codex with Caveman mode preloaded
- `prodex claude caveman` launches Claude Code with Caveman mode preloaded
- `prodex claude` runs Claude Code through the same profile pool

## Requirements

- At least one logged-in Prodex profile
- Codex CLI for `prodex` and `prodex caveman`
- Claude Code (`claude`) for `prodex claude` and `prodex claude caveman`

Installing `@christiandoxa/prodex` from npm also installs the Codex runtime dependency for you. Claude Code is still a separate CLI.

## Install

### npm

```bash
npm install -g @christiandoxa/prodex
```

### Cargo

```bash
cargo install prodex
```

Version-pinned install:

```bash
npm install -g @christiandoxa/prodex@0.10.0
cargo install prodex --force --version 0.10.0
```

## Quick Start

If your shared Codex home already contains a login:

```bash
prodex profile import-current main
```

Or create profiles through the normal login flow:

```bash
prodex login
prodex profile add second
prodex login --profile second
```

Inspect the pool:

```bash
prodex profile list
prodex quota --all
```

Run through Prodex:

```bash
prodex
prodex caveman
prodex exec "review this repo"
prodex claude -- -p "summarize this repo"
prodex claude caveman -- -p "summarize this repo briefly"
```

`prodex` without a subcommand is shorthand for `prodex run`.

## Main Commands

### Profiles

```bash
prodex profile list
prodex profile add second
prodex profile import-current main
prodex login --profile second
prodex use --profile main
prodex logout --profile main
prodex profile remove second
prodex profile remove --all
```

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
prodex caveman --profile main
prodex caveman exec "review this repo in caveman mode"
prodex caveman 019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9
```

`prodex caveman` uses the Caveman plugin from [JuliusBrussee/caveman](https://github.com/JuliusBrussee/caveman) and launches Codex from a temporary overlay `CODEX_HOME`, so the base profile home stays unchanged after the session ends.

### Claude Code

```bash
prodex claude -- -p "summarize this repo"
prodex claude caveman
prodex claude caveman -- -p "summarize this repo briefly"
prodex claude --profile second caveman -- -p "review the latest diff briefly"
prodex claude --profile second -- -p --output-format json "show the latest diff"
```

Use `prodex claude` for the normal Claude Code path, and use `prodex claude caveman` when you want the same Claude front end with Caveman mode preloaded.

Prefixing Claude args with `caveman` loads the Caveman plugin for that Claude session only while keeping Claude state under Prodex-managed `CLAUDE_CONFIG_DIR`, so the global `~/.claude` state is not the source of truth for the Prodex session.

### Export, Quota, and Debugging

```bash
prodex profile export
prodex quota --all
prodex quota --all --once
prodex doctor --runtime
```

## Learn More

For a longer walkthrough and the broader command set, see [QUICKSTART.md](./QUICKSTART.md).
