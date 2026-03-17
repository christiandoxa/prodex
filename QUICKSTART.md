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

## Debug the Environment

```bash
prodex doctor
prodex doctor --quota
```

## Notes

- `prodex` is only a wrapper; login is still handled by `codex`
- `prodex login` without `--profile` auto-creates or reuses a unique profile derived from the logged-in email
- that auto-create flow relies on being able to read the ChatGPT account email from `tokens.id_token` in `auth.json`
- built-in quota checks only work for profiles using ChatGPT auth
- managed Prodex profiles share Codex session history, so `/resume` shows the same saved threads across accounts
- `prodex run` performs quota preflight unless you use `--skip-quota-check`
- a profile is only treated as ready when both `5h` and `weekly` quota windows exist and still have remaining capacity
- `prodex run` auto-rotates to the next ready profile when the current one hits a limit, including when you pass `--profile`
- use `--no-auto-rotate` if you want the selected profile to stay blocked instead
