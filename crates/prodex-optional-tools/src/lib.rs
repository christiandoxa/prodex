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
pub const CAVEMAN_VETTED_VERSION: &str = "2.7.0";
pub const CAVEMAN_VETTED_COMMIT: &str = "8b0c1d3699b8d83e87fe4605b378da20c41555e0";
pub const CAVEMAN_VETTED_TREE_SHA256: &str =
    "09127915a13a493146ed0392b6895bbbd5f620d276dda9f4e68a6722f96df950";
pub(crate) const CAVEMAN_LEGACY_MANIFEST_TREE_SHA256: &str =
    "26d587fc179e79f76f4e2b42edec0266a7af40cf08bf15eb4609de310fabd8fb";
pub const PONYTAIL_VETTED_VERSION: &str = "4.10.0";
pub const PONYTAIL_VETTED_COMMIT: &str = "1d95ff7d39de12d87014ea40d4e22201bddc501b";
pub const PONYTAIL_VETTED_TREE_SHA256: &str =
    "05fe532f2a310cc7d12a60d6b51d1638a7d1465598d82ffa9f6a3d4cbf970f48";
pub(crate) const PONYTAIL_LEGACY_MANIFEST_TREE_SHA256: &str =
    "5443a5ee4a7248adcb59e1e102dd5bbd14af3083a9c3b4f271dd86790ac88c9c";
pub(crate) const PLAYWRIGHT_MCP_PACKAGE: &str = "@playwright/mcp@0.0.82";
pub(crate) const RTK_RECOMMENDED_VERSION: &str = "0.49.0";
pub(crate) const CODEBASE_MEMORY_RECOMMENDED_VERSION: &str = "0.11.0";
pub(crate) const PRESIDIO_RECOMMENDED_VERSION: &str = "2.2.364";

pub fn optional_tool_recommended_version(id: OptionalToolId) -> &'static str {
    match id {
        OptionalToolId::Caveman => CAVEMAN_VETTED_VERSION,
        OptionalToolId::Rtk => RTK_RECOMMENDED_VERSION,
        OptionalToolId::CodebaseMemoryMcp => CODEBASE_MEMORY_RECOMMENDED_VERSION,
        OptionalToolId::PlaywrightMcp => PLAYWRIGHT_MCP_PACKAGE
            .rsplit_once('@')
            .map(|(_, version)| version)
            .unwrap_or("unknown"),
        OptionalToolId::Ponytail => PONYTAIL_VETTED_VERSION,
        OptionalToolId::Presidio => PRESIDIO_RECOMMENDED_VERSION,
    }
}

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
