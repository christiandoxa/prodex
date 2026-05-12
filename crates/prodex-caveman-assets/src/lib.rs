mod embedded_files;
mod embedded_tree;
mod hook_trust;
mod launch_home;
mod localization;
mod marketplace;
mod rtk;
mod toml_helpers;

pub use embedded_tree::install_claude_caveman_plugin;
pub use hook_trust::trust_claude_mem_codex_plugin_hooks;
pub use launch_home::{configure_caveman_launch_home, prepare_caveman_launch_home};
pub use marketplace::{
    caveman_marketplace_root, install_caveman_marketplace, install_caveman_plugin_cache,
};
pub use rtk::configure_rtk_codex_home;

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
pub(crate) const AGENTS_MD: &str = "AGENTS.md";
pub(crate) const PRODEX_RTK_CODEX_AWARENESS: &str = r#"# RTK - Rust Token Killer (Codex CLI)

RTK is a token-optimized CLI proxy for shell commands.

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

#[cfg(test)]
use std::{env, fs, path::PathBuf};

#[cfg(test)]
#[path = "../tests/src/lib.rs"]
mod tests;
