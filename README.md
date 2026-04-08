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

Not everyone wants to pay $200 for a single account.

Sometimes it makes more sense to run 2 or 3 separate $20 accounts you already have. The problem is that doing it manually is annoying fast. You have to log in, log out, switch homes, check quota, and keep track of which session belongs to which account.

`prodex` exists to remove that pain.

It manages isolated profiles for each account, checks quota before launch, routes fresh work to an available profile, and keeps ongoing continuations attached to the right one.

Less account juggling, less friction, more actual work.

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
```

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

The current local version in this repo is `0.2.130`:

```bash
npm install -g @christiandoxa/prodex@0.2.130
cargo install prodex --force --version 0.2.130
```

Dependency status in this repo:

- The npm runtime dependency is already at the latest published `@openai/codex` release: `0.118.0`
- `cargo update` currently produces no Rust lockfile changes on the Rust `1.94.1` compatible graph used by this project
- `generic-array` remains pinned transitively by `crypto-common`, and `sha2 0.11` would require a wider RustCrypto compatibility jump than this release

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
prodex audit
prodex audit --tail 20 --component profile
prodex cleanup
prodex doctor
prodex doctor --quota
prodex doctor --runtime
prodex doctor --runtime --json
```

If a runtime session looks stalled, inspect the latest runtime log:

```bash
prodex doctor --runtime
tail -n 200 "$(cat /tmp/prodex-runtime-latest.path)"
```

That pointer path lives in `/tmp` only when you keep the default runtime log directory. If you override the runtime log directory through policy or environment, use `prodex doctor --runtime --json` to read the active `log_path` and live broker metrics.

Use `prodex cleanup` to remove stale local runtime logs, temporary login homes, dead broker leases and registries, plus old orphaned managed profile homes that are no longer tracked in state.

## Runtime Policy

Enterprise-style local deployments can pin runtime logging and proxy tuning in `$PRODEX_HOME/policy.toml` or `~/.prodex/policy.toml`.

```toml
version = 1

[runtime]
log_format = "json"
log_dir = "runtime-logs"

[secrets]
backend = "file"
# keyring_service = "prodex"

[runtime_proxy]
worker_count = 16
active_request_limit = 128
responses_active_limit = 96
http_connect_timeout_ms = 5000
stream_idle_timeout_ms = 300000
```

Notes:

* Environment variables still win over `policy.toml`.
* `prodex info` and `prodex doctor` show the active policy file, selected secret backend, and effective runtime log mode.
* The default runtime log format remains `text`; set `log_format = "json"` or `PRODEX_RUNTIME_LOG_FORMAT=json` when you want machine-readable runtime logs.
* Secret backend selection can be overridden with `PRODEX_SECRET_BACKEND` and `PRODEX_SECRET_KEYRING_SERVICE`.
* `prodex audit` reads the local append-only audit log and supports `--tail`, `--component`, `--action`, `--outcome`, and `--json`.

## Enterprise Hardening

The current hardening is still local-first, but it now includes:

- a secret-management abstraction for `auth.json` and exported profile bundles, plus global secret-backend selection via policy or environment
- a stable live broker snapshot at `GET /__prodex/runtime/metrics`
- a Prometheus scrape target at `GET /__prodex/runtime/metrics/prometheus`
- `prodex info` and `prodex doctor --runtime --json` surfacing live broker metrics targets and the selected secret backend
- enterprise audit logging for profile selection, rotation decisions, and admin-facing state changes, kept separate from transport behavior and discoverable via `prodex info` or `prodex doctor --runtime --json`
- `prodex audit` as a local read-only CLI surface for browsing recent append-only audit events

Current limitations:

- local `auth.json` remains the compatibility source of truth for current Codex flows even when a non-file backend is selected
- there is no keychain, Vault, or KMS-backed secret backend implementation yet
- audit logs follow the resolved runtime log directory by default, or `PRODEX_AUDIT_LOG_DIR` when set
- there is no central control plane, RBAC, SSO, or SCIM
- `prodex doctor --runtime --json` is operationally useful, but it is not a full observability stack
- the repo still assumes a per-host profile pool and local state ownership
- runtime-store modularization is still in progress, so persistence and audit/event handling remain implementation details rather than a public API

## Notes

* Managed profiles share persisted Codex state through Prodex-owned shared storage.
* `prodex quota --all` refreshes live by default.
* Use `--once` if you only want a one-shot snapshot.

## Learn More

For a longer walkthrough, see [QUICKSTART.md](./QUICKSTART.md).
