mod embedded_files;
mod embedded_tree;
mod hook_trust;
mod launch_home;
mod localization;
mod marketplace;
mod rtk;
mod super_optimizers;
mod toml_helpers;

pub use embedded_tree::install_claude_caveman_plugin;
pub use hook_trust::trust_claude_mem_codex_plugin_hooks;
pub use launch_home::{configure_caveman_launch_home, prepare_caveman_launch_home};
pub use marketplace::{
    caveman_marketplace_root, install_caveman_marketplace, install_caveman_plugin_cache,
};
pub use rtk::configure_rtk_codex_home;
pub use super_optimizers::configure_super_optimizer_codex_home;

pub const PRODEX_CAVEMAN_MARKETPLACE_NAME: &str = "prodex-caveman";
pub const PRODEX_CAVEMAN_PLUGIN_NAME: &str = "caveman";
pub const PRODEX_CAVEMAN_PLUGIN_VERSION: &str = "0.1.0";
pub const PRODEX_CAVEMAN_PLUGIN_ID: &str = "caveman@prodex-caveman";
pub const PRODEX_CAVEMAN_SOURCE_REPO: &str = "https://github.com/JuliusBrussee/caveman.git";
pub const PRODEX_CAVEMAN_FULL_ASSETS_ENV: &str = "PRODEX_CAVEMAN_FULL_ASSETS";
pub const PRODEX_CLAUDE_CAVEMAN_PLUGIN_NAME: &str = "caveman";
pub const PRODEX_RTK_SOURCE_REPO: &str = "https://github.com/rtk-ai/rtk.git";

pub(crate) const CLAUDE_MEM_PLUGIN_NAME: &str = "claude-mem";
pub(crate) const PRODEX_CAVEMAN_HOOK_TIMEOUT_SEC: u64 = 600;
pub(crate) const PRODEX_CAVEMAN_HOOK_COMMAND: &str = "echo 'CAVEMAN MODE ACTIVE. $caveman full: terse, no filler, exact tech. Code/commits/security normal. Stop: stop caveman/normal mode.'";
pub(crate) const RTK_MD: &str = "RTK.md";
pub(crate) const SUPER_OPTIMIZERS_MD: &str = "SUPER_OPTIMIZERS.md";
pub(crate) const AGENTS_MD: &str = "AGENTS.md";
pub(crate) const PRODEX_RTK_CODEX_AWARENESS: &str = r#"# RTK - Rust Token Killer (Codex CLI)

RTK is a token-optimized CLI proxy for shell commands.

## Role

RTK works on the upstream/input side. Use it before terminal output enters the model context: `git diff`, `cargo test`, `npm test`, `pytest`, build logs, and similar command output.

## Rule

Prefix shell commands with `rtk` when RTK is available.

Examples:

```bash
rtk git status
rtk cargo test
rtk npm run build
rtk pytest -q
```

If `rtk` is not installed or `rtk gain` fails, run the command raw and tell the user RTK is unavailable.

## Meta Commands

```bash
rtk gain
rtk gain --history
rtk proxy <cmd>
```

## Verification

```bash
rtk --version
rtk gain
which rtk
```
"#;

pub(crate) const PRODEX_SUPER_OPTIMIZER_AWARENESS: &str = r#"# Prodex Super Optimizers

Prodex Super mode already enables Caveman, RTK guidance, Claude-Mem super-slim recall, and Smart Context Autopilot. Use extra token optimizers only when they are local, deterministic, and safe for the current task.

## Token Flow

RTK handles upstream/input command output before it enters the context window. Use `rtk <cmd>` for noisy shell commands such as diffs, tests, builds, and package-manager output.

SQZ handles downstream/context reuse after content is already in the session. If the `prodex-sqz` MCP server is available, use it for repeated workspace reads, large text blobs, and conversation/context compression instead of re-emitting full text.

## Repeated Reads

Prefer existing artifact refs and Smart Context summaries over asking for the same file or command output again. If the `prodex-sqz` MCP server is available, use it for repeated workspace reads instead of emitting full repeated content.

## Code Navigation

If the `prodex-token-savior` MCP server is available, prefer its symbol/navigation tools before reading large source files. Keep exact source for edits, failing tests, stack traces, migrations, generated files, lockfiles, and security-sensitive changes.

Prodex registers `prodex-sqz` when `sqz-mcp` is on `PATH` or under a managed optimizer checkout, and `prodex-token-savior` when `token-savior` is on `PATH` or under a managed optimizer checkout. Managed roots are checked in this order: `PRODEX_OPTIMIZERS_HOME`, `XDG_DATA_HOME/prodex-optimizers`, then `~/.local/share/prodex-optimizers`. Missing binaries are skipped silently so Super still launches cleanly.

## AST Compression

If `claw-compactor` is installed, use it only as a manual, reversible code-summary aid for exploration. Do not edit from compressed code alone; rehydrate or reread the exact source before changing behavior.

## Documentation

Prefer checked-in or user-supplied `llm-min.txt` / SKF-style documentation when present. Do not run documentation minifiers that call Gemini, LLMs, SLMs, embeddings, or remote APIs unless the user explicitly asks.

## Safety

Never compress away critical signals: errors, panics, denied permissions, test failures, stack traces, diffs, review findings, secrets, auth material, quota/runtime proxy diagnostics, or exact command output that the user asked to see.
"#;

#[cfg(test)]
use std::{env, fs, path::PathBuf};

#[cfg(test)]
#[path = "../tests/src/lib.rs"]
mod tests;
