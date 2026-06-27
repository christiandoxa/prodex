mod asset_verification;
mod embedded_files;
mod embedded_tree;
mod hook_trust;
mod launch_home;
mod localization;
mod marketplace;
mod rtk;
mod super_optimizers;
mod toml_helpers;

use anyhow::Result;

pub use embedded_tree::install_claude_caveman_plugin;
pub use launch_home::{
    configure_caveman_launch_home, configure_prodex_overlay_home, prepare_caveman_launch_home,
    prepare_prodex_overlay_home, prepare_prodex_overlay_home_from_prepared_base,
};
pub use marketplace::{
    caveman_marketplace_root, install_caveman_marketplace, install_caveman_plugin_cache,
};
pub use rtk::configure_rtk_codex_home;
pub use super_optimizers::{
    SuperOptimizerMemoryConfig, configure_super_optimizer_codex_home,
    configure_super_optimizer_codex_home_with_options,
    configure_super_optimizer_codex_home_with_presidio,
};

pub const PRODEX_CAVEMAN_MARKETPLACE_NAME: &str = "prodex-caveman";
pub const PRODEX_CAVEMAN_PLUGIN_NAME: &str = "caveman";
pub const PRODEX_CAVEMAN_PLUGIN_VERSION: &str = "0.1.0";
pub const PRODEX_CAVEMAN_PLUGIN_ID: &str = "caveman@prodex-caveman";
pub const PRODEX_CAVEMAN_SOURCE_REPO: &str = "https://github.com/JuliusBrussee/caveman.git";
pub const PRODEX_CAVEMAN_FULL_ASSETS_ENV: &str = "PRODEX_CAVEMAN_FULL_ASSETS";
pub const PRODEX_CLAUDE_CAVEMAN_PLUGIN_NAME: &str = "caveman";
pub const PRODEX_RTK_SOURCE_REPO: &str = "https://github.com/rtk-ai/rtk.git";

pub(crate) const PRODEX_CAVEMAN_HOOK_TIMEOUT_SEC: u64 = 600;
pub(crate) const PRODEX_CAVEMAN_HOOK_COMMAND: &str = "prodex-caveman-sessionstart";
pub(crate) const PRODEX_CAVEMAN_HOOK_SCRIPT: &str = "prodex-caveman-sessionstart";
pub(crate) const PRODEX_CAVEMAN_HOOK_MARKER: &str = ".prodex-hooks/caveman-sessionstart";
pub(crate) const RTK_MD: &str = "RTK.md";
pub(crate) const SUPER_OPTIMIZERS_MD: &str = "SUPER_OPTIMIZERS.md";
pub(crate) const AGENTS_MD: &str = "AGENTS.md";
pub(crate) const PRODEX_RTK_CODEX_AWARENESS: &str = r#"# RTK - Rust Token Killer (Codex CLI)

RTK is a token-optimized CLI proxy for shell commands.

## Role

RTK works on the upstream/input side. Use it before terminal output enters the model context: `git diff`, `cargo test`, `npm test`, `pytest`, build logs, and similar command output.

## Rule

The `prodex rtk`, `prodex s`, or `prodex super` launch is the user instruction to use RTK. Do not wait for the user to ask again.

Hard rule: the visible shell command for noisy terminal work must begin with `rtk <cmd>`. This includes diffs, commit inspection, tests, builds, package-manager output, recursive search, and long logs.

Do not write plain noisy commands such as `git show ...`, `cargo test ...`, `npm run ...`, or `pytest ...`. Prodex Super also puts an overlay `rtk` wrapper first on PATH and auto-wraps common noisy commands when RTK is installed, but that wrapper is only a safety net. It does not make the Codex TUI/transcript show the `rtk` prefix and can be bypassed by inherited wrapper-depth environment.

Examples:

```bash
rtk git status
rtk cargo test
rtk npm run build
rtk pytest -q
```

If `rtk` is not installed or `rtk gain` fails, tell the user RTK is unavailable. Do not pretend RTK compression happened.

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

Prodex Super mode already enables Caveman, Ponytail, RTK, built-in prodex-inspect, SQZ, token-savior, claw-compactor, and Smart Context Autopilot when the matching local tools are installed. Treat launch through `prodex s` or `prodex super` as the user's instruction to use the local optimizer stack where it fits the task. Presidio redaction and prodex-memory are opt-in surfaces.

## Token Flow

Use the optimizers by default, but keep their boundaries clear:

- RTK handles upstream/input command output before it enters the context window. Visible noisy shell commands must use `rtk <cmd>` for diffs, commit inspection, tests, builds, package-manager output, recursive search, and long logs. Prodex also auto-wraps common noisy commands as a safety fallback when RTK is installed, but auto-wrappers are only a backstop for accidental misses. They are not a substitute for writing visible `rtk <cmd>` commands, because the Codex TUI shows the command text before PATH resolution.
- prodex-inspect handles read-only Prodex diagnostics through MCP. Use it for profile status, active profile, and latest runtime-log tail before shelling out to broad diagnostics.
- SQZ handles downstream/context reuse after content is already in the session. If the `prodex-sqz` MCP server is available, use it for repeated workspace reads, large pasted/generated text, long command outputs that must be reused, and conversation/context compression instead of re-emitting full text.
- token-savior handles codebase navigation and symbol context. If the `prodex-token-savior` MCP server is available, prefer it before reading broad source trees, hunting definitions, or scanning callers; then reread exact source for edits and failing lines.
- claw-compactor handles workspace-level Markdown/code-memory summaries. Use `prodex-claw-compactor` or `prodex-claw-compactor-auto` for explicit workspace summary/benchmark requests or when a large repo overview is needed; do not edit from compressed code alone.
- Ponytail is an optional local Codex plugin from `DietrichGebert/ponytail`. When available, follow its smallest-correct-implementation pressure: YAGNI, reuse, stdlib/native features, installed deps, then minimal custom code. Do not cut validation, security, accessibility, or required behavior.
- prodex-memory handles local-first memory through the `prodex-memory` MCP server when memory is requested. Use it for stable user preferences, project facts, and reusable context. It uses SQLite under `PRODEX_HOME` for the `mem` prefix, can use managed Mem0 OSS when Super launch opts in, and does not use Mem0 Cloud or require `MEM0_API_KEY`.

## Invocation Discipline

Before emitting or requesting large context, choose the local optimizer that fits:

- First pass over noisy terminal output: use visible `rtk <cmd>`.
- Prodex status/profile/runtime-log diagnostics: use `prodex-inspect`.
- Reusing content already seen, repeated file reads, or large text blobs: use `prodex-sqz` when available.
- Locating symbols, callers, dead code, or API changes: use `prodex-token-savior` when available.
- Workspace-level summary, benchmark, or memory-file compaction: use `prodex-claw-compactor`/`prodex-claw-compactor-auto` when available.
- Minimal implementation pressure: follow Ponytail when available; report if the plugin checkout is missing instead of pretending it is active.
- Durable local memory: use `prodex-memory` only when it is enabled for the session.

If a requested optimizer command or MCP server is unavailable, say so briefly and continue with the best local fallback. Do not pretend optimization happened.

## Installed Surfaces

Prodex registers built-in `prodex-inspect`, `prodex-sqz` when `sqz-mcp` is on `PATH` or under a managed optimizer checkout, `prodex-token-savior` when `token-savior` is on `PATH` or under a managed optimizer checkout, Ponytail when a `ponytail` checkout with `.codex-plugin/plugin.json` exists under a managed optimizer root, and `prodex-memory` from the running Prodex binary only when memory is requested. Managed roots are checked in this order: `PRODEX_OPTIMIZERS_HOME`, `XDG_DATA_HOME/prodex-optimizers`, then `~/.local/share/prodex-optimizers`. Missing binaries/checkouts are skipped silently so Super still launches cleanly. Prodex routes compatible optional-tool cache/state and local memory under `PRODEX_HOME` (default `~/.prodex`) instead of the workspace; managed Mem0 mode still keeps Mem0 server data under `PRODEX_HOME`.

## Workspace Hygiene

Optional optimizer state belongs under `PRODEX_HOME`, not in the user's current repository. Do not run initializer, installer, hook-writing, auto-merge, auto-compress, or memory-file mutation commands such as `rtk init`, `sqz init`, `claw-compactor install`, `claw-compactor auto`, or `--auto-merge` unless the user explicitly asked for that persistent workspace change. Reading, benchmarking, and summarizing the workspace is fine; creating or editing project files is not.

## AST Compression

If `claw-compactor` is available, Prodex Super installs a trusted one-shot SessionStart wrapper at `prodex-claw-compactor-sessionstart`. The startup probe is disabled by default so Codex launch is not delayed; opt in with `PRODEX_CLAW_SESSIONSTART_TIMEOUT_SECONDS=<seconds>` when you want the runtime to receive a compact workspace savings signal. The wrapper delegates to `prodex-claw-compactor-auto "$(pwd)"` only when that timeout is greater than zero. When the current directory has no Markdown memory files, the wrapper generates a temporary shadow workspace with a synthetic `MEMORY.md` and leaves the original directory untouched. Treat claw output as an overview or planning aid; rehydrate or reread exact source before changing behavior.

## Safety

Never compress away critical signals: errors, panics, denied permissions, test failures, stack traces, diffs, review findings, secrets, auth material, quota/runtime proxy diagnostics, or exact command output that the user asked to see. For exact-output tasks, bypass lossy compression and return the exact requested text.
"#;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CavemanAssetVerification {
    pub codex_plugin_files: usize,
    pub claude_plugin_files: usize,
    pub skill_files: usize,
}

pub fn verify_embedded_caveman_assets() -> Result<CavemanAssetVerification> {
    asset_verification::verify_embedded_caveman_assets()
}

#[cfg(test)]
use std::{env, fs, path::PathBuf};

#[cfg(test)]
#[path = "../tests/src/lib.rs"]
mod tests;
