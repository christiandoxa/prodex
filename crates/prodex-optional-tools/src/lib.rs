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

pub const RTK_MINIMUM_SUPPORTED_VERSION: &str = "0.46.0";
pub const RTK_LATEST_STABLE_REFERENCE: &str = "1.1.0";

pub const CODEBASE_MEMORY_MINIMUM_SUPPORTED_VERSION: &str = "0.9.1-rc.1";
pub const CODEBASE_MEMORY_LATEST_STABLE_REFERENCE: &str = "0.12.1";

pub const PLAYWRIGHT_MCP_MINIMUM_SUPPORTED_VERSION: &str = "0.0.79";
pub const PLAYWRIGHT_MCP_LATEST_STABLE_REFERENCE: &str = "0.0.83";
pub(crate) const PLAYWRIGHT_MCP_PACKAGE: &str = "@playwright/mcp";

pub const PONYTAIL_MINIMUM_SUPPORTED_VERSION: &str = "4.9.0";
pub const PONYTAIL_LATEST_STABLE_REFERENCE: &str = "5.3.3";
pub const PONYTAIL_LATEST_STABLE_COMMIT: &str = "7801d061243d7b753bad0e0b3c057cd250b11755";
pub const PONYTAIL_LATEST_STABLE_TREE_SHA256: &str =
    "3bd59937f9375a418f9f85c92b30b5d1404491578b784afa76d32ab2c1173af3";
pub(crate) const PONYTAIL_PREVIOUS_STABLE_REFERENCE: &str = "4.10.0";
pub(crate) const PONYTAIL_PREVIOUS_STABLE_COMMIT: &str = "1d95ff7d39de12d87014ea40d4e22201bddc501b";
pub(crate) const PONYTAIL_PREVIOUS_STABLE_TREE_SHA256: &str =
    "05fe532f2a310cc7d12a60d6b51d1638a7d1465598d82ffa9f6a3d4cbf970f48";
pub(crate) const PONYTAIL_PREVIOUS_LEGACY_MANIFEST_TREE_SHA256: &str =
    "5443a5ee4a7248adcb59e1e102dd5bbd14af3083a9c3b4f271dd86790ac88c9c";

pub const PRESIDIO_MINIMUM_SUPPORTED_VERSION: &str = "2.2.364";
pub const PRESIDIO_LATEST_STABLE_REFERENCE: &str = "2.2.366";

pub fn optional_tool_recommended_version(id: OptionalToolId) -> &'static str {
    match id {
        OptionalToolId::Rtk => RTK_LATEST_STABLE_REFERENCE,
        OptionalToolId::CodebaseMemoryMcp => CODEBASE_MEMORY_LATEST_STABLE_REFERENCE,
        OptionalToolId::PlaywrightMcp => PLAYWRIGHT_MCP_LATEST_STABLE_REFERENCE,
        OptionalToolId::Ponytail => PONYTAIL_LATEST_STABLE_REFERENCE,
        OptionalToolId::Presidio => PRESIDIO_LATEST_STABLE_REFERENCE,
    }
}

pub fn optional_tool_minimum_supported_version(id: OptionalToolId) -> &'static str {
    match id {
        OptionalToolId::Rtk => RTK_MINIMUM_SUPPORTED_VERSION,
        OptionalToolId::CodebaseMemoryMcp => CODEBASE_MEMORY_MINIMUM_SUPPORTED_VERSION,
        OptionalToolId::PlaywrightMcp => PLAYWRIGHT_MCP_MINIMUM_SUPPORTED_VERSION,
        OptionalToolId::Ponytail => PONYTAIL_MINIMUM_SUPPORTED_VERSION,
        OptionalToolId::Presidio => PRESIDIO_MINIMUM_SUPPORTED_VERSION,
    }
}

pub(crate) const RTK_MD: &str = "RTK.md";
pub(crate) const SUPER_OPTIMIZERS_MD: &str = "SUPER_OPTIMIZERS.md";
pub(crate) const PRODEX_RTK_CODEX_AWARENESS: &str = r#"# RTK - Rust Token Killer (Codex CLI)

RTK is a token-optimized CLI proxy for shell commands.

Use visible `rtk <cmd>` for noisy terminal work when RTK is installed. If it is unavailable,
report that accurately and run the underlying command normally.
"#;

#[cfg(test)]
#[path = "../tests/src/lib.rs"]
mod tests;
