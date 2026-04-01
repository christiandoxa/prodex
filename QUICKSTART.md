# Quick Start

Install from npm:

```bash
npm install -g @christiandoxa/prodex
```

This is usually the lightest option because it does not need a local Rust build.

Or install from [crates.io](https://crates.io/crates/prodex):

```bash
cargo install prodex
```

The npm package version is kept in sync with the published crate version on crates.io.

## Update Tips

Check your installed version first:

```bash
prodex --version
```

The current local binary version in this repo is `0.2.97`, so matching update commands look like this:

```bash
npm install -g @christiandoxa/prodex@0.2.97
cargo install prodex --force --version 0.2.97
```

If you prefer the lighter install path, npm is usually faster than `cargo install` because it skips the local build step.

If you want to switch from a Cargo-installed binary to npm, uninstall the Cargo binary first and then install the npm package:

```bash
cargo uninstall prodex
npm install -g @christiandoxa/prodex
```

## Use the Installed Binary

```bash
prodex
```

Prodex-owned commands adapt to the current terminal width, and live quota views can also adapt to terminal height when needed.

## Import your current `codex` login

If `~/.codex` already contains an active login:

```bash
prodex profile import-current main
```

## Or log in and let `prodex` create the profile

```bash
prodex login
```

`prodex login` creates a managed profile from the logged-in account email, reuses the same profile if that email is already known, and makes that profile active.

If the email-derived name is already taken by another account, `prodex` adds a numeric suffix instead of merging the two accounts into one profile.

If you want to log in to a specific profile name instead:

```bash
prodex profile add second
prodex login --profile second
```

Use `--profile` when you want a fixed profile name, or when the login flow is not the ChatGPT account flow that writes an email-bearing `id_token`.

`prodex login --device-auth` is also passed through unchanged when you need device-code auth.

## Check all profiles and quotas

```bash
prodex profile list
prodex info
prodex quota --all
```

`prodex profile list` renders a `Profiles` panel. `prodex quota --all` refreshes continuously by default every 5 seconds and renders a `Quota Overview` section with aggregated `5h` and `weekly` pool remaining above the `REMAINING` table and wrapped `status:` line for each profile.

`prodex info` renders a compact fleet summary with profile count, the installed prodex version and whether it is up to date, whether other `prodex` processes are currently running, aggregated `5h` and `weekly` quota pool remaining, and a no-reset runway estimate derived from active runtime logs when recent quota decay is observable.

The `REMAINING` column shows quota left, not quota used.

Press `Ctrl+C` to stop the live refresh loop.

The previous snapshot stays visible while the next refresh is loading.

If the terminal is too short to show every profile, the live `--all` view keeps the current sort order, shows only the top rows that fit, and summarizes how many profiles are hidden.

Use `prodex quota --all --detail` when you want the exact local reset timestamp for the `5h` and `weekly` windows under each profile row.

Use `prodex quota --all --once` when you want a single snapshot instead of continuous refresh.

## Select the active profile

```bash
prodex use --profile main
prodex current
```

`prodex current` renders the same responsive panel format, which is also used by `prodex doctor`, `prodex login`, and detailed single-profile quota views.

## Run `codex` through `prodex`

```bash
prodex
prodex run
```

Running `prodex` without a subcommand is shorthand for `prodex run`.

Examples:

```bash
prodex exec "review this repo"
prodex run -- --version
prodex run exec "review this repo"
printf 'context from stdin' | prodex run exec "summarize this"
prodex run 019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9
prodex run --profile second
prodex run --profile second --no-auto-rotate
```

`prodex run` uses a local runtime proxy for the session. The proxy keeps WebSocket and HTTP/SSE behavior as close as possible to direct `codex`, while handling safe account rotation before a request or stream is committed.

If the first positional argument looks like a Codex session id, `prodex run <session-id>` forwards to `codex resume <session-id>`.

In practice, that means:

- existing chains stay pinned through `previous_response_id` and `x-codex-turn-state`
- session-scoped unary routes such as `/responses/compact` also honor `session_id -> profile` affinity when a session owner is already known
- temporary quota, overload, and transport failures are tracked separately
- short-lived profile health penalties are endpoint-specific, so compact or websocket flakiness does not automatically poison fresh responses selection
- new candidate selection is also load-aware, so one profile is less likely to become a hotspot when several terminals are active
- fresh pre-commit selection also respects a short per-profile in-flight cap, so new work fails fast instead of piling onto a busy account
- that cap only applies to fresh pre-commit selection and does not override hard affinity for an existing continuation
- local proxy admission is also lane-aware, so `responses`, `compact`, `websocket`, and other unary traffic are less likely to starve each other
- lane-aware admission is also only for fresh local admission; it does not override hard affinity for an existing continuation
- flaky profiles can be deprioritized briefly without rotating mid-stream
- pre-commit retry/selection is bounded so the proxy does not keep spinning too long when every candidate is currently bad
- if a fresh request only exhausts local selection heuristics, the proxy may make one last direct attempt on the current profile
- generic upstream `429 Too Many Requests` responses are passed through; they only trigger safe rotation when the upstream payload explicitly reports `insufficient_quota` or `rate_limit_exceeded`
- if no healthy upstream profile can be secured before any upstream response exists, the proxy returns local `503 service_unavailable` instead of synthesizing a quota `429`
- `/responses/compact` also gets the same safe retry/rotate treatment for temporary overload or quota exhaustion
- `session_id` affinity is persisted in Prodex state, so compact and other session-scoped unary routes can keep using the owning profile after a proxy restart

## Debug the Environment

```bash
prodex doctor
prodex doctor --quota
prodex doctor --runtime
```

If a runtime session looks stalled, inspect the latest proxy log:

```bash
prodex doctor --runtime
tail -n 200 "$(cat /tmp/prodex-runtime-latest.path)"
```

Good markers to look for:

- `runtime_proxy_queue_overloaded`
- `runtime_proxy_active_limit_reached`
- `runtime_proxy_lane_limit_reached`
- `runtime_proxy_overload_backoff`
- `profile_inflight_saturated`
- `profile_retry_backoff`
- `profile_transport_backoff`
- `profile_inflight`
- `profile_health`
- `precommit_budget_exhausted`
- `first_upstream_chunk`
- `first_local_chunk`
- `stream_read_error`

If `profile_health` appears, also check its `route=` value before changing selection behavior globally.
If `runtime_proxy_lane_limit_reached` appears, inspect its `lane=` value first.
Repeated `lane=responses` markers suggest the main model lane is saturated locally; repeated non-`responses` markers usually mean a side lane is consuming proxy capacity.
If `runtime_proxy_active_limit_reached` or `profile_inflight_saturated` appears repeatedly without matching quota or transport markers, suspect local concurrency pressure first.
If you hit `exceeded retry limit, last status: 429 Too Many Requests`, check whether the runtime log actually shows upstream quota markers before assuming the account pool is exhausted.

## Notes

- `prodex` is only a wrapper; login is still handled by `codex`
- `prodex` without a subcommand behaves like `prodex run`
- `prodex login` without `--profile` auto-creates or reuses a unique profile derived from the logged-in email
- that auto-create flow first tries to read the ChatGPT account email from `tokens.id_token` in `auth.json`, then falls back to the usage endpoint email when needed
- built-in quota checks only work for profiles using ChatGPT auth
- managed Prodex profiles share selected native Codex state from the default `~/.codex`, including session history, rules, skills, config, and memories, so rotation stays closer to direct `codex`
- `prodex run <session-id>` is a shortcut for resuming a saved Codex session through the Prodex proxy
- `prodex run` performs quota preflight unless you use `--skip-quota-check`
- a profile is only treated as ready when both `5h` and `weekly` quota windows exist and still have remaining capacity
- `prodex run` auto-rotates to the next ready profile when the current one hits a limit, including when you pass `--profile`
- use `--no-auto-rotate` if you want the selected profile to stay blocked instead
- runtime proxy diagnostics are written to `/tmp/prodex-runtime-*.log`, with the latest path stored in `/tmp/prodex-runtime-latest.path`
