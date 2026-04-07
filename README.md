# prodex

[![CI](https://github.com/christiandoxa/prodex/actions/workflows/ci.yml/badge.svg)](https://github.com/christiandoxa/prodex/actions/workflows/ci.yml)

`prodex` manages multiple isolated Codex profiles and lets Codex CLI or Claude Code run on top of the same OpenAI account pool.

It is built for a simple setup:

- each account gets its own profile
- quota is checked before launch
- fresh work can move to another ready profile
- existing continuations stay on the profile that already owns them

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

The current local version in this repo is `0.2.124`:

```bash
npm install -g @christiandoxa/prodex@0.2.124
cargo install prodex --force --version 0.2.124
```

If you want to switch from a Cargo-installed binary to npm:

```bash
cargo uninstall prodex
npm install -g @christiandoxa/prodex
```

## Quick Setup

If your shared Codex home already contains a login:

```bash
prodex profile import-current main
```

Or create a profile through the normal login flow:

```bash
prodex login
prodex login --device-auth
```

If you want to name the profile first:

```bash
prodex profile add second
prodex login --profile second
```

Check the pool:

```bash
prodex profile list
prodex quota --all
prodex info
```

Run Codex CLI or Claude Code through Prodex:

```bash
prodex
prodex exec "review this repo"
prodex claude -- -p "summarize this repo"
```

`prodex` without a subcommand is shorthand for `prodex run`.

## Important Commands

### Profile And Login

```bash
prodex profile list
prodex profile add second
prodex profile import-current main
prodex login
prodex login --profile second
prodex login --device-auth
prodex use --profile main
prodex current
prodex logout --profile main
prodex profile remove second
```

### Run With Codex CLI

```bash
prodex
prodex run
prodex run --profile main
prodex exec "review this repo"
prodex run 019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9
printf 'context from stdin' | prodex run exec "summarize this"
```

### Run With Claude Code

```bash
prodex claude -- -p "summarize this repo"
prodex claude --profile second -- -p --output-format json "show the latest diff"
```

### Export And Import Profiles

```bash
prodex profile export
prodex profile export backup.json
prodex profile export --profile main --profile second backup.json
prodex profile import backup.json
```

`prodex profile export` includes each exported profile's `auth.json`. By default it exports every configured profile and asks whether the bundle should be password-protected.

### Quota, Status, And Debugging

```bash
prodex quota --all
prodex quota --all --once
prodex quota --profile main --detail
prodex info
prodex doctor
prodex doctor --quota
prodex doctor --runtime
```

If a runtime session looks stalled, inspect the latest runtime log:

```bash
prodex doctor --runtime
tail -n 200 "$(cat /tmp/prodex-runtime-latest.path)"
```

## Notes

- Managed profiles share persisted Codex state through Prodex-owned shared storage.
- `prodex quota --all` refreshes live by default. Use `--once` for a one-shot snapshot.

## More

For a longer walkthrough, see [QUICKSTART.md](./QUICKSTART.md).
