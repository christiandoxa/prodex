# Quick Start

Install from [crates.io](https://crates.io/crates/prodex):

```bash
cargo install prodex
```

## Use the Installed Binary

```bash
prodex
```

## Import your current `codex` login

If `~/.codex` already contains an active login:

```bash
prodex profile import-current main
```

## Or create a new profile and log in

```bash
prodex profile add second
prodex login --profile second
```

## Check all profiles and quotas

```bash
prodex profile list
prodex quota --all
```

## Select the active profile

```bash
prodex use main
prodex current
```

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
- built-in quota checks only work for profiles using ChatGPT auth
- `prodex run` performs quota preflight unless you use `--skip-quota-check`
- a profile is only treated as ready when both `5h` and `weekly` quota windows exist and still have remaining capacity
- `prodex run` auto-rotates to the next ready profile when the current one hits a limit, including when you pass `--profile`
- use `--no-auto-rotate` if you want the selected profile to stay blocked instead
