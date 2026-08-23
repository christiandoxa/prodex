mod caveman;
mod discovery;
mod fs_ops;
mod launch_home;
mod localization;
mod optional_tools;
mod process;
mod rtk;
mod super_optimizers;
mod toml_helpers;
mod tree;

pub use caveman::{activate_caveman_for_codex, resolve_caveman, resolve_caveman_claude_plugin_dir};
pub use launch_home::{
    configure_prodex_overlay_home, prepare_desktop_overlay_home,
    prepare_desktop_overlay_home_from_prepared_base, prepare_prodex_overlay_home,
    prepare_prodex_overlay_home_from_prepared_base, prepare_runtime_overlay_home,
    prepare_runtime_overlay_home_from_prepared_base,
};
pub use localization::{effective_agents_path, remove_agents_block, upsert_agents_block};
pub use optional_tools::{
    OptionalToolId, OptionalToolSet, ResolvedTool, ToolActivation, ToolActivationPlan,
    ToolCapability, ToolDescriptor, ToolDiscoverySource, ToolHealth, ToolHealthStatus, ToolKind,
    optional_tool_descriptor, optional_tool_status, resolve_optional_tools,
    resolve_optional_tools_for_launch,
};
pub use rtk::configure_rtk_codex_home;
pub use super_optimizers::{
    activate_optional_tools_for_codex, configure_super_optimizer_codex_home,
    configure_super_optimizer_codex_home_with_presidio,
};

pub const PRODEX_OPTIMIZERS_HOME_ENV: &str = "PRODEX_OPTIMIZERS_HOME";
pub const CAVEMAN_VETTED_VERSION: &str = "2.2.0";
pub const CAVEMAN_VETTED_COMMIT: &str = "9aa63945a349bef17206540650db48c30fafbdf2";
pub const CAVEMAN_VETTED_TREE_SHA256: &str =
    "91b4549bf361b2aed5ff0d131062788a8c672a941efe9e3db41beecb24a4112a";
pub const PONYTAIL_VETTED_VERSION: &str = "4.9.0";
pub const PONYTAIL_VETTED_COMMIT: &str = "0a4dd63ad4541f4f655c4108a295916f3c1d8fda";
pub const PONYTAIL_VETTED_TREE_SHA256: &str =
    "88c6dfa10bc0a63385a8f3f01bc4a3e51963c8fd76a0ebc0426bd889f0705970";
pub(crate) const PLAYWRIGHT_MCP_PACKAGE: &str = "@playwright/mcp@0.0.79";

pub(crate) const RTK_MD: &str = "RTK.md";
pub(crate) const SUPER_OPTIMIZERS_MD: &str = "SUPER_OPTIMIZERS.md";
pub(crate) const PRODEX_RTK_CODEX_AWARENESS: &str = r#"# RTK - Rust Token Killer (Codex CLI)

RTK is a token-optimized CLI proxy for shell commands.

Use visible `rtk <cmd>` for noisy terminal work when RTK is installed. If it is unavailable,
report that accurately and run the underlying command normally.
"#;
pub(crate) const PRODEX_SUPER_OPTIMIZER_AWARENESS: &str = r#"# Prodex Optional Tools

Prodex resolved optional tools for this temporary launch overlay.

- Use visible `rtk <cmd>` for noisy shell output when RTK is available.
- When Codebase Memory MCP is available, use it first for architecture, call-chain, impact,
  and structural code search; run `index_repository` first when the workspace is not indexed.
- Use Playwright MCP for browser work when available.
- Follow Ponytail when its plugin is active.
- Treat Presidio as enabled only when the session status says so.

Missing tools are not active. Do not claim they were used.
"#;

#[cfg(test)]
#[path = "../tests/src/lib.rs"]
mod tests;
