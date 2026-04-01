# prodex

Safe multi-account auto-rotate for `codex`.

`prodex` wraps `codex` with isolated profiles, built-in quota checks, and seamless account rotation.

## Why prodex

- auto-rotate to another ready account when the current one is quota-blocked or temporarily unhealthy
- keep each account isolated in its own `CODEX_HOME`
- preserve continuation affinity so ongoing chains stay on the right account
- keep transport behavior close to direct `codex`

## Install

Install from [crates.io](https://crates.io/crates/prodex):

```bash
cargo install prodex
```

Or, once the npm package is published, install the native wrapper from npm:

```bash
npm install -g @christiandoxa/prodex
```

The npm package version is kept in lockstep with the published crate version.

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

Run `codex` with safe auto-rotate:

```bash
prodex run
```

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

## Core Commands

```bash
prodex profile list
prodex use main
prodex info
prodex quota --all
prodex quota --all --once
prodex doctor
prodex doctor --runtime
prodex run
prodex run 019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9
```

## Notes

- one `prodex` profile = one isolated `CODEX_HOME`
- `prodex login` still uses the real `codex login` flow, including `--device-auth`
- `prodex login` without `--profile` first tries to read the ChatGPT account email from `tokens.id_token` in `auth.json`, then falls back to the usage endpoint email when needed
- `prodex run <session-id>` forwards to `codex resume <session-id>`
- `prodex info` summarizes profile count, the installed prodex version and update status, running Prodex processes, aggregated quota pool, and a no-reset runway estimate from active runtime logs
- `prodex quota` live-refreshes every 5 seconds by default, and `prodex quota --all` also shows aggregated `5h` and `weekly` pool remaining before the per-profile table
- Prodex-owned screens adapt to terminal width, and live views can also adapt to terminal height

For a slightly longer setup guide, see [QUICKSTART.md](./QUICKSTART.md).
