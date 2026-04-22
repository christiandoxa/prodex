# prodex

`prodex` is a wrapper for Codex and Claude Code when you want to work with multiple isolated profiles.

Each profile can use a different account. Before starting a session, `prodex` checks quota and can route new work to another available profile. Existing sessions stay attached to the profile they started with.

It can also run Caveman mode and optionally connect Claude-Mem to the active session path.

## When to use it

`prodex` is useful if you:

- use multiple accounts for CLI agent workflows
- want isolated profile environments
- need quota checks before launch
- want session continuity tied to the original profile

If you only use one account and do not care about quota-aware routing or isolated homes, you probably do not need it.

## Repository layout

- `src/main.rs` — binary entrypoint
- `src/lib.rs` — shared crate wiring and test support
- `src/app_commands/`, `src/command_dispatch.rs`, `src/cli_args.rs` — CLI parsing and top-level command flow
- `src/profile_commands/`, `src/quota_support/`, `src/secret_store/`, `src/profile_identity.rs` — profile, quota, secret storage, and identity management
- `src/runtime_proxy/`, `src/runtime_launch/`, `src/runtime_persistence/`, `src/runtime_store/`, `src/runtime_broker/` — runtime internals
- `src/runtime_claude/`, `src/runtime_anthropic/`, `src/runtime_caveman.rs`, `src/runtime_mem.rs` — Claude, Caveman, and memory integrations
- `scripts/npm/` and `npm/` — npm packaging and publishing helpers

## Supported commands

- `prodex` / `prodex run`
- `prodex caveman`
- `prodex caveman mem`
- `prodex claude`
- `prodex claude caveman`
- `prodex claude caveman mem`

## Requirements

You need at least one logged-in Prodex profile.

Depending on your setup, you may also need:

- Codex CLI for `prodex` and `prodex caveman`
- Claude Code for `prodex claude` and `prodex claude caveman`
- `claude-mem` for `mem` variants

## Install

### npm

```bash
npm install -g @christiandoxa/prodex
````

This installs `prodex` and pulls in the current Codex runtime dependency.

### Cargo

```bash
cargo install prodex
```

Cargo installs use the `codex` binary already available in your `PATH`, so you need to keep that updated separately.

## Quick start

If your current Codex home is already logged in:

```bash
prodex profile import-current main
```

Or set it up from scratch:

```bash
prodex login
prodex profile add second
prodex login --profile second
```

Import a logged-in Copilot CLI account:

```bash
prodex profile import copilot
prodex profile import copilot --name copilot-main --activate
```

Check available profiles and quota:

```bash
prodex profile list
prodex quota --all
```

Run commands through `prodex`:

```bash
prodex
prodex caveman
prodex caveman mem
prodex exec "review this repo"
prodex claude -- -p "summarize this repo"
prodex claude mem -- -p "recall past work on this repo"
prodex claude caveman -- -p "summarize this repo briefly"
prodex claude caveman mem -- -p "summarize this repo briefly"
```

## Profile commands

```bash
prodex profile list
prodex profile add second
prodex profile import copilot
prodex profile import-current main
prodex login --profile second
prodex use --profile main
prodex logout --profile main
prodex profile remove second
prodex profile remove --all
```

### Note on Copilot import

`prodex profile import copilot` does not move the Copilot token into Prodex-managed storage. The token stays where Copilot already stores it. Prodex only records the provider identity and API endpoint in its own metadata.

## Codex examples

```bash
prodex
prodex run
prodex run --profile main
prodex exec "review this repo"
prodex run 019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9
printf 'context from stdin' | prodex run exec "summarize this"
```

## Caveman examples

```bash
prodex caveman
prodex caveman mem
prodex caveman --profile main
prodex caveman exec "review this repo in caveman mode"
prodex caveman 019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9
```

`prodex caveman` runs Codex with a temporary overlay `CODEX_HOME`, so the base profile home stays unchanged after the session ends.

If you use the `mem` variant, Prodex points an existing Claude-Mem Codex setup to the active Prodex session path instead of the default `~/.codex/sessions`.

## Claude Code examples

```bash
prodex claude -- -p "summarize this repo"
prodex claude mem -- -p "recall past work on this repo"
prodex claude caveman
prodex claude caveman mem
prodex claude caveman -- -p "summarize this repo briefly"
prodex claude caveman mem -- -p "summarize this repo briefly"
prodex claude --profile second caveman -- -p "review the latest diff briefly"
prodex claude --profile second -- -p --output-format json "show the latest diff"
```

`prodex claude` uses the normal Claude Code flow.

`prodex claude caveman` loads Caveman only for that session while keeping state under the Prodex-managed `CLAUDE_CONFIG_DIR`, not the global `~/.claude`.

`prodex claude caveman mem` combines both Caveman and Claude-Mem.

## Utility commands

```bash
prodex profile export
prodex quota --all
prodex quota --all --once
prodex doctor --runtime
```

If Prodex returns `409 stale_continuation`, the request still carries continuation state, but the original binding is gone or no longer safe to reuse. Prodex refuses to fresh-replay that turn on another profile because continuation affinity is part of the request contract, and replaying it elsewhere can drop tool context or continue the wrong conversation. Start a new prompt or retry from the same live session if it still exists. If the failure looks unexpected, `prodex doctor --runtime` and the latest runtime log can show which continuation binding went stale.

## More

See [QUICKSTART.md](./QUICKSTART.md) for a longer walkthrough.
