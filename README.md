# prodex

Safe multi-account auto-rotate for `codex`.

`prodex` wraps `codex` with isolated profiles, built-in quota checks, and seamless account rotation.

## Why prodex

- auto-rotate to another ready account when the current one is quota-blocked or temporarily unhealthy
- keep each account isolated in its own `CODEX_HOME`
- preserve continuation affinity so ongoing chains stay on the right account
- keep transport behavior close to direct `codex`

## Install

```bash
cargo install prodex
```

Crate page: [crates.io/crates/prodex](https://crates.io/crates/prodex)

## Quick Start

Import your current `codex` login:

```bash
prodex profile import-current main
```

Or create a profile through `codex login`:

```bash
prodex login
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
- `prodex login` still uses the real `codex login` flow
- `prodex run <session-id>` forwards to `codex resume <session-id>`
- `prodex info` summarizes profile count, the installed prodex version and update status, running Prodex processes, aggregated quota pool, and a no-reset runway estimate from active runtime logs
- `prodex quota` live-refreshes every 5 seconds by default, and `prodex quota --all` also shows aggregated `5h` and `weekly` pool remaining before the per-profile table
- Prodex-owned screens adapt to terminal width, and live views can also adapt to terminal height

For a slightly longer setup guide, see [QUICKSTART.md](./QUICKSTART.md).
