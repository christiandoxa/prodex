use super::{RuntimeToolArgs, codex_args_with_feature_overrides};
use prodex_optional_tools::{OptionalToolId, OptionalToolSet};
use std::ffi::OsString;
use std::fmt;

pub(super) fn extract_super_leading_launch_prefixes(
    args: Vec<OsString>,
) -> (Vec<OptionalToolId>, Vec<OsString>) {
    let mut tools = Vec::new();
    let mut consumed = 0;
    for arg in &args {
        let Some(prefix) = arg.to_str() else {
            break;
        };
        let tool = match prefix {
            "rtk" => OptionalToolId::Rtk,
            "playwright" => OptionalToolId::PlaywrightMcp,
            "ponytail" => OptionalToolId::Ponytail,
            "presidio" => OptionalToolId::Presidio,
            _ => break,
        };
        tools.push(tool);
        consumed += 1;
    }
    (tools, args.into_iter().skip(consumed).collect())
}

impl fmt::Debug for RuntimeToolArgs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RuntimeToolArgs")
            .field("profile_configured", &self.profile.is_some())
            .field("auto_rotate", &self.auto_rotate)
            .field("no_auto_rotate", &self.no_auto_rotate)
            .field("auto_redeem", &self.auto_redeem)
            .field("skip_quota_check", &self.skip_quota_check)
            .field("full_access", &self.full_access)
            .field("dry_run", &self.dry_run)
            .field("base_url_configured", &self.base_url.is_some())
            .field("no_proxy", &self.no_proxy)
            .field("smart_context", &self.smart_context)
            .field("tools", &self.tools)
            .field("required_tools", &self.required_tools)
            .field("presidio", &self.presidio)
            .field("external_provider", &self.external_provider)
            .field("harness", &self.harness)
            .field(
                "external_provider_api_key",
                &self
                    .external_provider_api_key
                    .as_ref()
                    .map(|_| "<redacted>"),
            )
            .field("codex_features", &self.codex_features)
            .field("codex_args_count", &self.codex_args.len())
            .finish()
    }
}

impl RuntimeToolArgs {
    pub fn codex_args_with_feature_overrides(&self) -> Vec<OsString> {
        codex_args_with_feature_overrides(&self.codex_args, &self.codex_features)
    }

    pub fn select_tool(&mut self, tool: OptionalToolId) {
        if !self.tools.contains(&tool) {
            self.tools.push(tool);
        }
    }

    pub fn require_tool(&mut self, tool: OptionalToolId) {
        self.select_tool(tool);
        if !self.required_tools.contains(&tool) {
            self.required_tools.push(tool);
        }
    }

    pub fn selected_tool_set(&self) -> OptionalToolSet {
        self.tools
            .iter()
            .chain(&self.required_tools)
            .copied()
            .collect()
    }

    pub fn required_tool_set(&self) -> OptionalToolSet {
        self.required_tools.iter().copied().collect()
    }

    pub fn translate_legacy_leading_tool_prefixes(&mut self) {
        let mut translated = Vec::new();
        for arg in &self.codex_args {
            let Some(prefix) = arg.to_str() else {
                break;
            };
            let tool = match prefix {
                "caveman" => OptionalToolId::Caveman,
                "rtk" => OptionalToolId::Rtk,
                "playwright" => OptionalToolId::PlaywrightMcp,
                "ponytail" => OptionalToolId::Ponytail,
                "presidio" => OptionalToolId::Presidio,
                _ => break,
            };
            translated.push(tool);
        }
        for tool in &translated {
            self.select_tool(*tool);
            self.presidio |= *tool == OptionalToolId::Presidio;
        }
        self.codex_args.drain(..translated.len());
    }
}

pub fn runtime_tool_args_with_tool(
    mut args: RuntimeToolArgs,
    tool: OptionalToolId,
) -> RuntimeToolArgs {
    args.select_tool(tool);
    args
}
