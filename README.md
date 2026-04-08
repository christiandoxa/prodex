# prodex

[![CI](https://github.com/christiandoxa/prodex/actions/workflows/ci.yml/badge.svg)](https://github.com/christiandoxa/prodex/actions/workflows/ci.yml)

Run multiple isolated Codex profiles on the same OpenAI account pool, with smart quota checks, clean profile separation, and continuation-aware routing.

`prodex` helps you manage multiple Codex identities without turning your workflow into a mess.

It is designed around a simple model:

- **One account = one profile**
- **Quota is checked before launch**
- **Fresh work can move to another ready profile**
- **Existing continuations stay with the profile that already owns them**

That means you can keep working smoothly across multiple accounts while preserving session continuity where it matters.

## Why `prodex`?

If you use Codex CLI or Claude Code heavily, account switching and quota limits can become painful fast.

`prodex` solves that by:

- isolating each account into its own profile
- checking quota before a session starts
- letting new work land on another available profile
- keeping ongoing continuations attached to their original profile

The result is a workflow that feels predictable, lightweight, and safe.

## Requirements

Before using `prodex`, make sure you have:

- **An OpenAI account**, plus at least one logged-in Prodex profile
- **Codex CLI**, if you want to use `prodex`
- **Claude Code (`claude`)**, if you want to use `prodex claude`

> Installing `@christiandoxa/prodex` from npm also installs the Codex runtime dependency for you.  
> Claude Code is still a separate CLI and must already be available on your `PATH` if you want to use `prodex claude`.

## Install

### npm

```bash
npm install -g @christiandoxa/prodex
````

### Cargo

```bash
cargo install prodex
```

The npm package version is kept in lockstep with the published crate version.

## Update

Check your installed version:

```bash
prodex --version
```

The current local version in this repo is `0.2.126`:

```bash
npm install -g @christiandoxa/prodex@0.2.126
cargo install prodex --force --version 0.2.126
```

Switching from a Cargo-installed binary to npm?

```bash
cargo uninstall prodex
npm install -g @christiandoxa/prodex
```

## Quick Setup

If your shared Codex home already contains a login:

```bash
prodex profile import-current main
```

Or create a profile through the usual login flow:

```bash
prodex login
prodex login --device-auth
```

Want to name the profile first?

```bash
prodex profile add second
prodex login --profile second
```

Check your profile pool and quota status:

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

> `prodex` without a subcommand is shorthand for `prodex run`.

## Common Workflows

### 1. Create or import profiles

```bash
prodex profile import-current main
prodex profile add second
prodex login --profile second
```

### 2. Inspect your pool

```bash
prodex profile list
prodex quota --all
prodex info
```

### 3. Run Codex with automatic profile selection

```bash
prodex
prodex run
prodex exec "review this repo"
```

### 4. Resume an existing session on the correct profile

```bash
prodex run 019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9
```

### 5. Run Claude Code through the same profile pool

```bash
prodex claude -- -p "summarize this repo"
prodex claude --profile second -- -p --output-format json "show the latest diff"
```

## Command Reference

### Profile & Login

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

### Run with Codex CLI

```bash
prodex
prodex run
prodex run --profile main
prodex exec "review this repo"
prodex run 019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9
printf 'context from stdin' | prodex run exec "summarize this"
```

### Run with Claude Code

```bash
prodex claude -- -p "summarize this repo"
prodex claude --profile second -- -p --output-format json "show the latest diff"
```

### Export & Import Profiles

```bash
prodex profile export
prodex profile export backup.json
prodex profile export --profile main --profile second backup.json
prodex profile import backup.json
```

`prodex profile export` includes each exported profile’s `auth.json`.
By default, it exports every configured profile and asks whether the bundle should be password-protected.

### Quota, Status & Debugging

```bash
prodex quota --all
prodex quota --all --once
prodex quota --profile main --detail
prodex info
prodex cleanup
prodex doctor
prodex doctor --quota
prodex doctor --runtime
```

If a runtime session looks stalled, inspect the latest runtime log:

```bash
prodex doctor --runtime
tail -n 200 "$(cat /tmp/prodex-runtime-latest.path)"
```

Use `prodex cleanup` to remove stale local runtime logs, temporary login homes, dead broker leases and registries, plus old orphaned managed profile homes that are no longer tracked in state.

## Notes

* Managed profiles share persisted Codex state through Prodex-owned shared storage.
* `prodex quota --all` refreshes live by default.
* Use `--once` if you only want a one-shot snapshot.

## Learn More

For a longer walkthrough, see [QUICKSTART.md](./QUICKSTART.md).
