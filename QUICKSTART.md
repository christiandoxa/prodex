# Quick Start

Install from [crates.io](https://crates.io/crates/prodex):

```bash
cargo install prodex
```

## Use the Installed Binary

```bash
prodex
```

Prodex-owned commands use a fixed 110-character layout with section headers, wrapped key-value fields, and readable tables.

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

## Check all profiles and quotas

```bash
prodex profile list
prodex quota --all
```

`prodex profile list` renders a `Profiles` panel. `prodex quota --all` renders a `Quota Overview` table with a `REMAINING` column and a wrapped `status:` line for each profile.

The `REMAINING` column shows quota left, not quota used.

Use `prodex quota --all --detail` when you want the exact local reset timestamp for the `5h` and `weekly` windows under each profile row.

## Select the active profile

```bash
prodex use main
prodex current
```

`prodex current` renders the same fixed-width panel format, which is also used by `prodex doctor`, `prodex login`, and detailed single-profile quota views.

## Run `codex` through `prodex`

```bash
prodex run
```

Examples:

```bash
prodex run -- --version
prodex run exec "review this repo"
prodex run --profile second
prodex run --profile second --no-auto-rotate
```

`prodex run` uses a local runtime proxy for the session. The proxy keeps WebSocket and HTTP/SSE behavior as close as possible to direct `codex`, while handling safe account rotation before a request or stream is committed.

In practice, that means:

- existing chains stay pinned through `previous_response_id` and `x-codex-turn-state`
- temporary quota, overload, and transport failures are tracked separately
- short-lived profile health penalties are endpoint-specific, so compact or websocket flakiness does not automatically poison fresh responses selection
- new candidate selection is also load-aware, so one profile is less likely to become a hotspot when several terminals are active
- fresh pre-commit selection also respects a short per-profile in-flight cap, so new work fails fast instead of piling onto a busy account
- that cap only applies to fresh pre-commit selection and does not override hard affinity for an existing continuation
- local proxy admission is also lane-aware, so `responses`, `compact`, `websocket`, and other unary traffic are less likely to starve each other
- lane-aware admission is also only for fresh local admission; it does not override hard affinity for an existing continuation
- flaky profiles can be deprioritized briefly without rotating mid-stream
- pre-commit retry/selection is bounded so the proxy does not keep spinning too long when every candidate is currently bad
- `/responses/compact` also gets the same safe retry/rotate treatment for temporary overload or quota exhaustion

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

## Notes

- `prodex` is only a wrapper; login is still handled by `codex`
- `prodex login` without `--profile` auto-creates or reuses a unique profile derived from the logged-in email
- that auto-create flow relies on being able to read the ChatGPT account email from `tokens.id_token` in `auth.json`
- built-in quota checks only work for profiles using ChatGPT auth
- managed Prodex profiles share the default `~/.codex` session history store, so `/resume` shows the same saved threads across accounts
- `prodex run` performs quota preflight unless you use `--skip-quota-check`
- a profile is only treated as ready when both `5h` and `weekly` quota windows exist and still have remaining capacity
- `prodex run` auto-rotates to the next ready profile when the current one hits a limit, including when you pass `--profile`
- use `--no-auto-rotate` if you want the selected profile to stay blocked instead
- runtime proxy diagnostics are written to `/tmp/prodex-runtime-*.log`, with the latest path stored in `/tmp/prodex-runtime-latest.path`
