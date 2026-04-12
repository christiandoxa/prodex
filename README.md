# prodex

[![CI](https://github.com/christiandoxa/prodex/actions/workflows/ci.yml/badge.svg)](https://github.com/christiandoxa/prodex/actions/workflows/ci.yml)

Run multiple isolated Codex profiles on the same OpenAI account pool.

`prodex` is a small CLI for managing multiple Codex profiles without manually switching logins and local state.

It uses a simple model:

- **One account = one profile**
- **Quota is checked before launch**
- **Fresh work can move to another ready profile**
- **Existing continuations stay with the profile that already owns them**

## Why `prodex`?

If you use multiple accounts, doing everything by hand gets old quickly.

A common case is having 2 or 3 separate $20 accounts instead of paying for a higher-tier plan. That works, but the workflow is annoying. You have to log in and out, switch homes, check quota manually, and remember which session belongs to which account.

`prodex` handles that setup by keeping profiles isolated, checking quota before launch, and keeping continuations on the profile that already owns them.

## Requirements

Before using `prodex`, make sure you have:

- **An OpenAI account**, plus at least one logged-in Prodex profile
- **Codex CLI**, if you want to use `prodex`
- **Claude Code (`claude`)**, if you want to use `prodex claude`

> Installing `@christiandoxa/prodex` from npm also installs the Codex runtime dependency for you.  
> Claude Code is still a separate CLI and should already be available on your `PATH` when you use `prodex claude`.

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

The current local version in this repo is `0.9.0`:

```bash
npm install -g @christiandoxa/prodex@0.9.0
cargo install prodex --force --version 0.9.0
```

Dependency status in this repo:

* The npm runtime dependency follows the `latest` dist-tag declared in the workspace package manifest for `@openai/codex`
* Run `cargo update` whenever dependency metadata changes so the published lockfile stays in sync
* Versioned install snippets in this README and `QUICKSTART.md` are synced from `Cargo.toml`

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

If you want to name the profile first:

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

`prodex` without a subcommand is shorthand for `prodex run`.

## Common Workflows

### Create or import profiles

```bash
prodex profile import-current main
prodex profile add second
prodex login --profile second
```

### Inspect your pool

```bash
prodex profile list
prodex quota --all
prodex info
```

### Run Codex with automatic profile selection

```bash
prodex
prodex run
prodex exec "review this repo"
```

### Resume an existing session on the correct profile

```bash
prodex run 019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9
```

### Run Claude Code through the same profile pool

```bash
prodex claude -- -p "summarize this repo"
prodex claude --profile second -- -p --output-format json "show the latest diff"
```

For managed profiles, Prodex can seed Claude state from your existing `~/.claude` and `~/.claude.json` on first use, then keep Claude config and chat history in shared Prodex-managed state.

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

Use `prodex cleanup` to remove stale local runtime logs, temporary login homes, dead broker leases and registries, transient runtime cache files in `.prodex`, stale root temp files left by interrupted atomic writes, collapse duplicate profiles that resolve to the same account email into one surviving profile, plus old orphaned managed profile homes that are no longer tracked in state.

## Advanced Runtime Configuration

If you want tighter control over runtime logging, secrets, or proxy behavior, you can pin local settings in `$PRODEX_HOME/policy.toml` or `~/.prodex/policy.toml`.

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

* Environment variables still win over `policy.toml`
* `prodex info` and `prodex doctor` show the active policy file, selected secret backend, and effective runtime log mode
* The default runtime log format remains `text`; set `log_format = "json"` or `PRODEX_RUNTIME_LOG_FORMAT=json` when you want machine-readable runtime logs
* Secret backend selection can be overridden with `PRODEX_SECRET_BACKEND` and `PRODEX_SECRET_KEYRING_SERVICE`
* `prodex audit` reads the local append-only audit log and supports `--tail`, `--component`, `--action`, `--outcome`, and `--json`

## Hardening And Operational Notes

The current setup is still local-first, but it already includes:

* a secret-management abstraction for `auth.json` and exported profile bundles
* a stable live broker snapshot at `GET /__prodex/runtime/metrics`
* a Prometheus scrape target at `GET /__prodex/runtime/metrics/prometheus`
* `prodex info` and `prodex doctor --runtime --json` surfacing live broker metrics targets and the selected secret backend
* append-only audit logging for profile selection, rotation decisions, and admin-facing state changes
* `prodex audit` as a local read-only CLI surface for browsing recent audit events

Current limitations:

* local `auth.json` remains the compatibility source of truth for current Codex flows even when a non-file backend is selected
* there is no keychain, Vault, or KMS-backed secret backend implementation yet
* audit logs follow the resolved runtime log directory by default, or `PRODEX_AUDIT_LOG_DIR` when set
* there is no central control plane, RBAC, SSO, or SCIM
* `prodex doctor --runtime --json` is useful operationally, but it is not a full observability stack
* the repo still assumes a per-host profile pool and local state ownership
* runtime-store modularization is still in progress, so persistence and audit/event handling remain implementation details rather than a public API

## Notes

* Managed profiles share persisted Codex state through Prodex-owned shared storage
* `prodex quota --all` refreshes live by default
* Use `--once` if you only want a one-shot snapshot

## Learn More

For a longer walkthrough, see [QUICKSTART.md](./QUICKSTART.md).
