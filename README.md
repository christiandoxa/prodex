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

### 3. Add a new profile and log in

If you want a fresh empty profile:

```bash
prodex profile add second
prodex login --profile second
```

`prodex login` does not handle the OAuth callback itself. It only runs `codex login` with the selected profile's `CODEX_HOME`.

### 4. View all quotas

```bash
prodex quota --all
```

Example `MAIN` column:

```text
5h 37/100 used | weekly 12/100 used
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

## Quota Behavior

Before `prodex run` launches `codex`, `prodex` tries to check quota for the selected profile.

Before a profile is considered safe to use, `prodex` requires both the `5h` and `weekly` quota windows to be present and still below `100/100`.

If that profile does not clearly have remaining required quota:

- `prodex run` tries to rotate to the next ready profile by default, including when you pass `--profile`
- if you want the command to stay blocked on that profile instead, use `--no-auto-rotate`
- it prints the missing, unknown, or exhausted quota reasons
- it suggests other profiles that appear ready, when available

If auto-rotate succeeds, the active profile is updated to the profile that was used.

## Important Notes

- quota checks are built into `prodex` and use the ChatGPT backend endpoint used by Codex
- ChatGPT quota can only be read when the profile uses ChatGPT auth, not an API key
- if a profile uses API key auth, `quota --all` will show `error` for that profile
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
