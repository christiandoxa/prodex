use super::{
    RuntimeToolArgs, SuperArgs, codex_args_with_feature_overrides,
    super_external_provider_base_url, super_external_provider_codex_args,
    super_local_provider_base_url, super_local_provider_codex_args,
};
use crate::CodexRuntimeFeatureArgs;
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

impl SuperArgs {
    pub fn presidio_preference(&self) -> Option<bool> {
        if self.presidio {
            Some(true)
        } else if self.no_presidio {
            Some(false)
        } else {
            None
        }
    }

    pub fn into_runtime_tool_args(self) -> RuntimeToolArgs {
        self.into_runtime_tool_args_with_presidio(false)
    }

    pub fn into_runtime_tool_args_with_presidio(self, presidio: bool) -> RuntimeToolArgs {
        let (legacy_tools, passthrough_codex_args) =
            extract_super_leading_launch_prefixes(self.codex_args);
        let presidio = presidio || legacy_tools.contains(&OptionalToolId::Presidio);
        let local_upstream_base_url = self.url.as_deref().map(super_local_provider_base_url);
        let external_upstream_base_url = self.provider.map(|provider| {
            self.base_url
                .as_deref()
                .map(super_external_provider_base_url)
                .unwrap_or_else(|| provider.default_base_url().to_string())
        });
        let local_provider_args = self
            .url
            .as_deref()
            .map(|url| {
                super_local_provider_codex_args(
                    url,
                    self.local_model.as_deref(),
                    self.local_context_window,
                    self.local_auto_compact_token_limit,
                )
            })
            .unwrap_or_default();
        let external_provider_args = self
            .provider
            .map(|provider| {
                super_external_provider_codex_args(
                    provider,
                    external_upstream_base_url.as_deref().unwrap_or_default(),
                    self.local_model.as_deref(),
                    self.local_context_window,
                    self.local_auto_compact_token_limit,
                )
            })
            .unwrap_or_default();
        let local_mode = self.url.is_some() || self.provider.is_some();
        let skip_quota_check = self.skip_quota_check || local_mode;

        let feature_overrides = self.codex_features.to_codex_config_args();
        let mut codex_args = Vec::new();
        codex_args.extend(local_provider_args);
        codex_args.extend(external_provider_args);
        codex_args.extend(feature_overrides);
        codex_args.extend(passthrough_codex_args);
        let mut tools = OptionalToolSet::super_defaults();
        for tool in self.tools.into_iter().chain(legacy_tools) {
            tools.insert(tool);
        }
        if presidio {
            tools.insert(OptionalToolId::Presidio);
        }
        for tool in &self.required_tools {
            tools.insert(*tool);
        }
        RuntimeToolArgs {
            profile: self.profile,
            auto_rotate: self.auto_rotate,
            no_auto_rotate: self.no_auto_rotate,
            auto_redeem: self.auto_redeem,
            skip_quota_check,
            // Super is the explicit YOLO entrypoint. Keep the parsed --full-access flag for
            // compatibility, but never downgrade a Super launch when it is omitted.
            full_access: true,
            dry_run: self.dry_run,
            base_url: local_upstream_base_url
                .or(external_upstream_base_url)
                .or(self.base_url),
            no_proxy: self.no_proxy,
            smart_context: true,
            super_mode: true,
            tools: tools.iter().collect(),
            required_tools: self.required_tools,
            presidio,
            external_provider: self.provider,
            external_provider_api_key: self.api_key,
            harness: self.harness,
            codex_features: CodexRuntimeFeatureArgs::default(),
            codex_args,
        }
    }
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
            .field("super_mode", &self.super_mode)
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
