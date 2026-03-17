# Quick Start

## Build

```bash
cargo build --release
```

Use the binary:

```bash
./target/release/prodex
```

## Import your current `codex` login

If `~/.codex` already contains an active login:

```bash
./target/release/prodex profile import-current main
```

## Or create a new profile and log in

```bash
./target/release/prodex profile add second
./target/release/prodex login --profile second
```

## Check all profiles and quotas

```bash
./target/release/prodex profile list
./target/release/prodex quota --all
```

## Select the active profile

```bash
./target/release/prodex use main
./target/release/prodex current
```

## Run `codex` through `prodex`

```bash
./target/release/prodex run
```

Examples:

```bash
./target/release/prodex run -- --version
./target/release/prodex run exec "review this repo"
./target/release/prodex run --profile second
./target/release/prodex run --profile second --no-auto-rotate
```

## Debug the Environment

```bash
./target/release/prodex doctor
./target/release/prodex doctor --quota
```

## Notes

- `prodex` is only a wrapper; login is still handled by `codex`
- quota via `cq` only works for profiles using ChatGPT auth
- `prodex run` performs quota preflight unless you use `--skip-quota-check`
- a profile is only treated as ready when both `5h` and `weekly` quota windows exist and still have remaining capacity
- `prodex run` auto-rotates to the next ready profile when the current one hits a limit, including when you pass `--profile`
- use `--no-auto-rotate` if you want the selected profile to stay blocked instead
