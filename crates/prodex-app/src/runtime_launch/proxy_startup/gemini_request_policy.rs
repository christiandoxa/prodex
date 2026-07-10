use super::gemini_request::{
    RUNTIME_GEMINI_EXTENSION_SCAN_LIMIT, RUNTIME_GEMINI_MEMORY_BYTE_LIMIT, runtime_gemini_home_dir,
};
use super::gemini_request_extensions::runtime_gemini_active_extension_manifests;
use super::gemini_request_io::runtime_gemini_read_text_limited;
use prodex_provider_core::{
    gemini_provider_core_collect_string_values,
    gemini_provider_core_normalize_tool_name as runtime_gemini_normalize_tool_name,
    gemini_provider_core_parse_command_specific_tool,
    gemini_provider_core_tool_aliases as runtime_gemini_tool_aliases,
    gemini_provider_core_tool_call_command_text as runtime_gemini_tool_call_command_text,
    gemini_provider_core_tool_is_mutating as runtime_gemini_tool_is_mutating,
};
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Default)]
pub(super) struct RuntimeGeminiPolicyCompat {
    allowed_tools: Option<BTreeSet<String>>,
    excluded_tools: BTreeSet<String>,
    command_specific_exclusions: BTreeMap<String, BTreeSet<String>>,
    approval_mode: Option<String>,
}

impl RuntimeGeminiPolicyCompat {
    pub(super) fn from_request_and_files(original: &serde_json::Value) -> Self {
        let mut policy = Self::default();
        for path in runtime_gemini_settings_paths() {
            if let Some(text) =
                runtime_gemini_read_text_limited(&path, RUNTIME_GEMINI_MEMORY_BYTE_LIMIT)
                && let Some(value) = crate::parse_gemini_settings_json(&text)
            {
                policy.apply_settings_value(&value);
            }
        }
        for extension in runtime_gemini_active_extension_manifests() {
            policy.apply_settings_value(&extension.value);
            policy.apply_extension_policy_files(&extension.directory);
        }
        for key in ["gemini_policy", "geminiPolicy"] {
            if let Some(value) = original.get(key).filter(|value| !value.is_null()) {
                policy.apply_settings_value(value);
            }
        }
        policy
    }

    pub(super) fn apply_settings_value(&mut self, value: &serde_json::Value) {
        if let Some(mode) = value
            .pointer("/general/defaultApprovalMode")
            .or_else(|| value.pointer("/tools/approvalMode"))
            .or_else(|| value.get("approvalMode"))
            .and_then(serde_json::Value::as_str)
        {
            self.approval_mode = Some(mode.to_ascii_lowercase());
        }
        let mut excluded = Vec::new();
        gemini_provider_core_collect_string_values(value.pointer("/tools/exclude"), &mut excluded);
        gemini_provider_core_collect_string_values(value.get("excludeTools"), &mut excluded);
        for name in excluded {
            self.add_excluded_tool_or_command(&name);
        }

        let mut allowed = Vec::new();
        gemini_provider_core_collect_string_values(value.pointer("/tools/allowed"), &mut allowed);
        gemini_provider_core_collect_string_values(value.get("allowedTools"), &mut allowed);
        gemini_provider_core_collect_string_values(value.pointer("/tools/core"), &mut allowed);
        gemini_provider_core_collect_string_values(value.get("coreTools"), &mut allowed);
        if !allowed.is_empty() {
            let allowed_tools = self.allowed_tools.get_or_insert_with(BTreeSet::new);
            for name in allowed {
                allowed_tools.insert(runtime_gemini_normalize_tool_name(&name));
            }
        }
    }

    pub(super) fn apply_extension_policy_files(&mut self, directory: &Path) {
        let policies = directory.join("policies");
        let Ok(entries) = fs::read_dir(policies) else {
            return;
        };
        for entry in entries.flatten().take(RUNTIME_GEMINI_EXTENSION_SCAN_LIMIT) {
            let path = entry.path();
            if path.extension().and_then(|extension| extension.to_str()) != Some("toml") {
                continue;
            }
            let Some(text) =
                runtime_gemini_read_text_limited(&path, RUNTIME_GEMINI_MEMORY_BYTE_LIMIT)
            else {
                continue;
            };
            let Ok(value) = toml::from_str::<toml::Value>(&text) else {
                continue;
            };
            self.apply_extension_policy_toml(&value);
        }
    }

    pub(super) fn apply_extension_policy_toml(&mut self, value: &toml::Value) {
        let Some(rules) = value.get("rule").and_then(toml::Value::as_array) else {
            return;
        };
        for rule in rules {
            let decision = rule
                .get("decision")
                .and_then(toml::Value::as_str)
                .unwrap_or_default()
                .to_ascii_lowercase();
            if !matches!(decision.as_str(), "deny" | "block" | "blocked") {
                continue;
            }
            if let Some(tool_name) = rule.get("toolName").and_then(toml::Value::as_str) {
                if let Some(pattern) = runtime_gemini_policy_command_pattern_from_rule(rule) {
                    self.command_specific_exclusions
                        .entry(runtime_gemini_normalize_tool_name(tool_name))
                        .or_default()
                        .insert(pattern);
                } else {
                    self.add_excluded_tool_or_command(tool_name);
                }
            }
        }
    }

    fn add_excluded_tool_or_command(&mut self, value: &str) {
        if let Some((tool_name, pattern)) = gemini_provider_core_parse_command_specific_tool(value)
        {
            self.command_specific_exclusions
                .entry(runtime_gemini_normalize_tool_name(&tool_name))
                .or_default()
                .insert(pattern);
            return;
        }
        self.excluded_tools
            .insert(runtime_gemini_normalize_tool_name(value));
    }

    pub(super) fn filter_function_declarations(&self, declarations: &mut Vec<serde_json::Value>) {
        declarations.retain(|declaration| {
            declaration
                .get("name")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|name| self.tool_is_allowed(name))
        });
    }

    pub(super) fn tool_is_allowed(&self, name: &str) -> bool {
        let aliases = runtime_gemini_tool_aliases(name);
        if aliases
            .iter()
            .any(|alias| self.excluded_tools.contains(alias))
        {
            return false;
        }
        if self.approval_mode.as_deref() == Some("plan") && runtime_gemini_tool_is_mutating(name) {
            return false;
        }
        if let Some(allowed) = &self.allowed_tools {
            return aliases.iter().any(|alias| allowed.contains(alias));
        }
        true
    }

    pub(super) fn summary(&self) -> Option<String> {
        let mut lines = Vec::new();
        if let Some(mode) = &self.approval_mode {
            lines.push(format!("defaultApprovalMode: {mode}"));
        }
        if let Some(allowed) = &self.allowed_tools
            && !allowed.is_empty()
        {
            lines.push(format!(
                "allowed tools: {}",
                allowed.iter().cloned().collect::<Vec<_>>().join(", ")
            ));
        }
        if !self.excluded_tools.is_empty() {
            lines.push(format!(
                "excluded tools: {}",
                self.excluded_tools
                    .iter()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        if !self.command_specific_exclusions.is_empty() {
            let entries = self
                .command_specific_exclusions
                .iter()
                .map(|(tool, patterns)| {
                    format!(
                        "{}({})",
                        tool,
                        patterns.iter().cloned().collect::<Vec<_>>().join(", ")
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            lines.push(format!("excluded tool argument patterns: {entries}"));
        }
        (!lines.is_empty()).then(|| lines.join("\n"))
    }

    pub(super) fn blocked_tool_call_message(
        &self,
        name: &str,
        args: &serde_json::Value,
    ) -> Option<String> {
        let aliases = runtime_gemini_tool_aliases(name);
        if aliases
            .iter()
            .any(|alias| self.excluded_tools.contains(alias))
        {
            return Some(format!(
                "Gemini policy blocked tool call `{name}` because the tool is excluded."
            ));
        }
        let command = runtime_gemini_tool_call_command_text(args);
        for alias in aliases {
            let Some(patterns) = self.command_specific_exclusions.get(&alias) else {
                continue;
            };
            for pattern in patterns {
                if command_matches_policy_pattern(&command, pattern) {
                    return Some(format!(
                        "Gemini policy blocked tool call `{name}` because command `{command}` matched blocked pattern `{pattern}`."
                    ));
                }
            }
        }
        None
    }
}

fn command_matches_policy_pattern(command: &str, pattern: &str) -> bool {
    let command = command.to_ascii_lowercase();
    let pattern = pattern.to_ascii_lowercase();
    command.contains(pattern.trim())
}

fn runtime_gemini_policy_command_pattern_from_rule(rule: &toml::Value) -> Option<String> {
    for key in [
        "command",
        "pattern",
        "matches",
        "commandPattern",
        "command_pattern",
    ] {
        if let Some(pattern) = rule.get(key).and_then(toml::Value::as_str) {
            let pattern = pattern.trim();
            if !pattern.is_empty() {
                return Some(pattern.to_string());
            }
        }
    }
    None
}

fn runtime_gemini_settings_paths() -> Vec<PathBuf> {
    runtime_gemini_settings_paths_for(
        runtime_gemini_home_dir().as_deref(),
        env::current_dir().ok().as_deref(),
    )
}

pub(super) fn runtime_gemini_settings_paths_for(
    home: Option<&Path>,
    cwd: Option<&Path>,
) -> Vec<PathBuf> {
    crate::gemini_settings_source_paths_for(home, cwd)
        .into_iter()
        .map(|(_, path)| path)
        .collect()
}
