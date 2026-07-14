mod asset_verification;
mod embedded_files;
mod embedded_tree;
mod fs_ops;
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
    configure_super_optimizer_codex_home, configure_super_optimizer_codex_home_with_presidio,
    super_optimizer_command_ready, super_playwright_npx_command,
};

pub const PRODEX_CAVEMAN_MARKETPLACE_NAME: &str = "prodex-caveman";
pub const PRODEX_CAVEMAN_PLUGIN_NAME: &str = "caveman";
pub const PRODEX_CAVEMAN_PLUGIN_VERSION: &str = "0.1.0";
pub const PRODEX_CAVEMAN_PLUGIN_ID: &str = "caveman@prodex-caveman";
pub const PRODEX_CAVEMAN_SOURCE_REPO: &str = "https://github.com/JuliusBrussee/caveman.git";
pub const PRODEX_CAVEMAN_FULL_ASSETS_ENV: &str = "PRODEX_CAVEMAN_FULL_ASSETS";
pub const PRODEX_CLAUDE_CAVEMAN_PLUGIN_NAME: &str = "caveman";
pub const PRODEX_RTK_SOURCE_REPO: &str = "https://github.com/rtk-ai/rtk.git";

pub(crate) const RTK_MD: &str = "RTK.md";
pub(crate) const SUPER_OPTIMIZERS_MD: &str = "SUPER_OPTIMIZERS.md";
pub(crate) const AGENTS_MD: &str = "AGENTS.md";
pub(crate) const PRODEX_CAVEMAN_DEVELOPER_INSTRUCTIONS: &str = "CAVEMAN MODE ACTIVE. $caveman full: terse, no filler, exact tech. Code/commits/security normal. Stop: stop caveman/normal mode.\nPRODEX SUPER TOOLS ACTIVE WHEN AVAILABLE. Ponytail applies smallest-correct-implementation pressure. Use visible rtk <cmd> for noisy shell output and codebase-memory-mcp for structural code navigation. Presidio is opt-in only.";
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

pub(crate) const PRODEX_SUPER_OPTIMIZER_AWARENESS: &str = r#"# Prodex Super Tools

Prodex Super enables Caveman, Ponytail, RTK, Codebase Memory MCP, Playwright MCP, Smart Context, and optional Presidio.

- Use visible `rtk <cmd>` for noisy shell output.
- Use `codebase-memory-mcp` for architecture, call-chain, impact, and structural code search.
- Use Playwright for browser inspection and automation when its MCP server is available.
- Follow Ponytail's smallest-correct-implementation pressure when its plugin is available.
- Treat Presidio as enabled only when the session status says so.

Missing optional tools must not block the session. Do not claim a tool was used when it was unavailable.
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
