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
pub use optional_tools::{
    OptionalToolId, OptionalToolSet, ResolvedTool, ToolActivation, ToolActivationPlan,
    ToolCapability, ToolDescriptor, ToolDiscoverySource, ToolHealth, ToolHealthStatus, ToolKind,
    optional_tool_descriptor, optional_tool_status, resolve_optional_tools,
};
pub use rtk::configure_rtk_codex_home;
pub use super_optimizers::{
    activate_optional_tools_for_codex, configure_super_optimizer_codex_home,
    configure_super_optimizer_codex_home_with_presidio,
};

pub const PRODEX_OPTIMIZERS_HOME_ENV: &str = "PRODEX_OPTIMIZERS_HOME";
pub const CAVEMAN_VETTED_VERSION: &str = "1.9.1";
pub const CAVEMAN_VETTED_COMMIT: &str = "0d95a81d35a9f2d123a5e9430d1cfc43d55f1bb0";
pub const CAVEMAN_VETTED_TREE_SHA256: &str =
    "863d1a6965ed47f9e130312c8e943617e224cc08f8162296d7e06b8b63d54476";
pub const PONYTAIL_VETTED_VERSION: &str = "4.8.4";
pub const PONYTAIL_VETTED_COMMIT: &str = "16f29800fd2681bdf24f3eb4ccffe38be3baec6b";
pub const PONYTAIL_VETTED_TREE_SHA256: &str =
    "727ac132ab903b3abf46cabd3d8ee855984e83d6f8ef36665853604c9a5c2e7d";

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
- Use Codebase Memory MCP for structural code navigation when available.
- Use Playwright MCP for browser work when available.
- Follow Ponytail when its plugin is active.
- Treat Presidio as enabled only when the session status says so.

Missing tools are not active. Do not claim they were used.
"#;

#[cfg(test)]
#[path = "../tests/src/lib.rs"]
mod tests;
