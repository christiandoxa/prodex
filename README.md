# prodex

`prodex` is a CLI wrapper for `codex` that separates multiple profiles by giving each one its own `CODEX_HOME`.

Install from [crates.io](https://crates.io/crates/prodex):

```bash
cargo install prodex
```

For the shorter version, see [QUICKSTART.md](./QUICKSTART.md).

In short:

- one `prodex` profile = one `CODEX_HOME` directory
- login is still handled by `codex`
- `prodex` manages profiles, the active profile, built-in quota checks, and launching `codex`
- `prodex run` uses a local runtime proxy that keeps Codex transport behavior as intact as possible while rotating accounts safely
- Prodex-owned screens use a fixed 110-character layout with section headers, wrapped fields, and readable tables

The mental model is similar to browser profiles, but for `codex`.

## Quick Start

### 1. Install

```bash
cargo install prodex
```

Package page:

[https://crates.io/crates/prodex](https://crates.io/crates/prodex)

### 2. Import your current `codex` profile

If you already have an active login in `~/.codex`:

```bash
prodex profile import-current main
```

This will:

- copy `~/.codex` into a new managed profile
- store the profile in `~/.prodex/profiles/main`
- set `main` as the active profile

### 3. Log in and let `prodex` create the profile

If you want a fresh login, `prodex` can create or reuse a profile automatically from the account email:

```bash
prodex login
```

This will:

- run `codex login` in a temporary isolated `CODEX_HOME`
- resolve the logged-in account email from the ChatGPT `id_token` stored in `auth.json`
- create a managed profile whose name is derived from that email
- reuse the existing profile instead of creating a duplicate when that email is already registered
- switch the active profile to the reused or newly created profile

If the email-derived profile name is already taken by a different account, `prodex` keeps the email uniqueness rule and creates a suffixed name such as `main_example.com-2`.

If you want to target a specific existing profile name instead:

```bash
prodex profile add second
prodex login --profile second
```

Use `prodex login --profile <name>` when you want a fixed profile name, or when you are not using the ChatGPT login flow that writes an email-bearing `id_token`.

`prodex login` still delegates the actual authentication flow to `codex`.

### 4. View all quotas

```bash
prodex quota --all
```

`prodex quota --all` prints a `Quota Overview` table with a fixed-width layout. The main quota summary now appears in the `REMAINING` column, and each profile gets a wrapped `status:` detail line underneath the row.

If you also want the exact reset timestamps for the required main windows:

```bash
prodex quota --all --detail
```

This adds a `resets:` line under each profile row, including the full local timestamp for both `5h` and `weekly`.

Example `REMAINING` value:

```text
5h 63% left | weekly 88% left
```

### 5. Select the active profile and run `codex`

```bash
prodex use main
prodex run
```

Or run directly with a specific profile:

```bash
prodex run --profile second
```

## Requirements

`prodex` relies on the following binaries:

- `codex`

Quick check:

```bash
codex --help
```

If you want to audit the `prodex` environment:

```bash
prodex doctor
prodex doctor --quota
prodex doctor --runtime
```

## How It Works

`prodex` stores its own state in:

```text
~/.prodex
```

The main structure is:

- `state.json`: the list of profiles and the active profile
- `profiles/<name>`: the managed `CODEX_HOME` for each profile

Authentication is still stored by `codex` inside each profile's `auth.json`.

## Most Common Commands

### Profile Management

Install from crates.io:

```bash
cargo install prodex
```

Add an empty profile:

```bash
prodex profile add work
```

Import from `~/.codex`:

```bash
prodex profile import-current work
```

List all profiles:

```bash
prodex profile list
```

This renders a `Profiles` panel with wrapped fields and one panel per profile.

Select the active profile:

```bash
prodex use work
```

Remove a profile:

```bash
prodex profile remove work
```

Remove a profile and its managed home:

```bash
prodex profile remove work --delete-home
```

### Login/Logout

Log in and auto-create or reuse a unique profile based on the email you use:

```bash
prodex login
```

This only works when the login flow writes a ChatGPT `id_token` with an email claim into `auth.json`.

Log in to a specific profile:

```bash
prodex login --profile work
```

Log out from a specific profile:

```bash
prodex logout --profile work
```

### Quota

Show quota for one profile:

```bash
prodex quota --profile work
```

Show raw quota JSON:

```bash
prodex quota --profile work --raw
```

View all profiles at once:

```bash
prodex quota --all
```

This renders a `Quota Overview` table with `PROFILE`, `CUR`, `AUTH`, `ACCOUNT`, `PLAN`, and `REMAINING`, plus a `status:` line for each profile.

Add `--detail` to include exact local reset timestamps for `5h` and `weekly` under each row:

```bash
prodex quota --all --detail
```

### Run `codex`

Run `codex` with the active profile:

```bash
prodex run
```

Run `codex` with arguments:

```bash
prodex run -- --version
prodex run exec "review this repo"
```

Run with a specific profile:

```bash
prodex run --profile work
```

Temporarily disable auto-rotate:

```bash
prodex run --profile work --no-auto-rotate
```

Skip quota preflight:

```bash
prodex run --profile work --skip-quota-check
```

At runtime, `prodex` keeps the transport side as close as possible to direct `codex`:

- WebSocket and HTTP/SSE traffic still follow Codex reconnect and fallback behavior
- account rotation happens inside the proxy, but only before a request or stream is committed
- existing chains stay pinned through `previous_response_id` and `x-codex-turn-state`
- temporary quota, overload, or transport failures can move later requests to another ready profile without rotating mid-stream
- the current profile is still tried optimistically first when it looks healthy
- new candidate selection is also load-aware, so one profile is less likely to become a hotspot when several terminals are active at once
- fresh pre-commit selection also respects a short per-profile in-flight cap, so new work fails fast instead of piling more pressure onto an already busy account
- that per-profile cap only applies to fresh pre-commit selection; it does not override hard affinity for an existing continuation
- local proxy admission is also lane-aware, so bursty `compact`, `responses`, `websocket`, and other unary traffic are less likely to starve each other
- lane-aware admission is also only for fresh local admission; it does not override hard affinity for an existing continuation
- quota backoff, transport backoff, and short-lived profile health penalties are tracked separately so the proxy can stop hammering a flaky account without weakening hard affinity
- short-lived profile health penalties are endpoint-specific, so a hot `/responses/compact` path or flaky WebSocket route does not automatically poison fresh `responses` selection
- pre-commit candidate selection is bounded, so when all candidates are currently bad the proxy fails fast instead of spinning in the background for too long
- when a fresh request only exhausts local selection heuristics, the proxy may make one last direct attempt on the current profile before giving up
- generic upstream `429 Too Many Requests` responses are passed through; they are not treated as account-specific quota unless the upstream payload explicitly reports `insufficient_quota` or `rate_limit_exceeded`
- if the proxy cannot secure a healthy upstream profile before any upstream response exists, it now fails with local `503 service_unavailable` instead of synthesizing a local quota `429`
- the unary compact path (`/responses/compact`) is also eligible for safe retry and rotation on temporary overload or quota exhaustion

## Quota Behavior

Before `prodex run` launches `codex`, `prodex` tries to check quota for the selected profile.

Before a profile is considered safe to use, `prodex` requires both the `5h` and `weekly` quota windows to be present and still have remaining capacity.

If that profile does not clearly have remaining required quota:

- `prodex run` tries to rotate to the next ready profile by default, including when you pass `--profile`
- if you want the command to stay blocked on that profile instead, use `--no-auto-rotate`
- it prints the missing, unknown, or exhausted quota reasons
- it suggests other profiles that appear ready, when available

If auto-rotate succeeds, the active profile is updated to the profile that was used.

## Runtime Diagnostics

If a session appears stalled or reconnect-heavy, inspect the latest runtime proxy log:

```bash
prodex doctor --runtime
cat /tmp/prodex-runtime-latest.path
tail -n 200 "$(cat /tmp/prodex-runtime-latest.path)"
```

Useful files:

- `/tmp/prodex-runtime-latest.path`: pointer to the latest runtime log
- `/tmp/prodex-runtime-*.log`: per-run transport, fallback, and rotation diagnostics

Useful log markers:

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
If `runtime_proxy_lane_limit_reached` appears, inspect its `lane=` value before changing upstream-facing behavior.
Repeated `lane=responses` markers suggest the main model lane is saturated locally; repeated non-`responses` markers usually mean a side lane is consuming proxy capacity.
If `runtime_proxy_active_limit_reached` or `profile_inflight_saturated` appears repeatedly without matching quota or transport markers, suspect local concurrency pressure before changing upstream-facing behavior.
If users report `exceeded retry limit, last status: 429 Too Many Requests`, confirm whether the latest runtime log shows real upstream quota markers or only local pre-commit exhaustion before changing rotation behavior.

## Important Notes

- quota checks are built into `prodex` and use the ChatGPT backend endpoint used by Codex
- ChatGPT quota can only be read when the profile uses ChatGPT auth, not an API key
- `prodex login` without `--profile` depends on being able to read the ChatGPT account email from `tokens.id_token` in `auth.json`
- if a profile uses API key auth, `quota --all` will show `error` for that profile
- managed Prodex profiles share the default `~/.codex` session history store, so `/resume` shows the same saved threads across accounts
- `profile list`, `current`, `doctor`, `login`, `quota`, and other Prodex-owned screens use the same 110-character layout
- runtime proxy diagnostics are written to `/tmp`, not to the Codex TUI
- `prodex` does not replace `codex`; it only acts as a launcher and profile manager

## Environment Variables

Override the `prodex` state location:

```bash
PRODEX_HOME=/path/to/prodex-home
```

Override the `codex` binary:

```bash
PRODEX_CODEX_BIN=/path/to/codex
```

Override the default ChatGPT quota base URL:

```bash
CODEX_CHATGPT_BASE_URL=https://chatgpt.com/backend-api
```

## Development

Run during development:

```bash
cargo run -- profile list
cargo run -- quota --all
cargo run -- doctor
```

Tests:

```bash
cargo test
```
