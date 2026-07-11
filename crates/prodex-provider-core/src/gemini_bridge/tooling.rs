//! Gemini tool declaration and tool-output guard helpers.

use std::collections::BTreeSet;

mod gemini3;
mod guardrails;

pub use self::gemini3::{
    gemini_provider_core_apply_gemini3_tool_declaration_overrides,
    gemini_provider_core_gemini3_tool_description, gemini_provider_core_model_uses_gemini3_toolset,
};
pub use self::guardrails::{
    gemini_provider_core_blocked_tool_call_item,
    gemini_provider_core_conversation_requests_command_output_only,
    gemini_provider_core_forced_command_output,
    gemini_provider_core_non_actionable_wait_or_poll_text,
    gemini_provider_core_tool_intent_without_call, gemini_provider_core_unverified_success_claim,
};

pub fn gemini_provider_core_tool_aliases(name: &str) -> BTreeSet<String> {
    let mut aliases = BTreeSet::new();
    let normalized = gemini_provider_core_normalize_tool_name(name);
    aliases.insert(normalized.clone());
    if let Some(suffix) = normalized.rsplit("__").next() {
        aliases.insert(suffix.to_string());
    }
    match normalized.as_str() {
        "exec_command" | "run_shell_command" | "shell" | "bash" => {
            aliases
                .extend(["exec_command", "run_shell_command", "shell", "bash"].map(str::to_string));
        }
        "apply_patch" | "edit" | "replace" => {
            aliases.extend(["apply_patch", "edit", "replace"].map(str::to_string));
        }
        "read_file" | "read" => {
            aliases.extend(["read_file", "read"].map(str::to_string));
        }
        "read_many_files" | "glob" => {
            aliases.extend(["read_many_files", "glob"].map(str::to_string));
        }
        "grep" | "rip_grep" | "rg" | "search" => {
            aliases.extend(["grep", "rip_grep", "rg", "search"].map(str::to_string));
        }
        "write_file" | "write" => {
            aliases.extend(["write_file", "write"].map(str::to_string));
        }
        _ => {}
    }
    aliases
}

pub fn gemini_provider_core_normalize_tool_name(name: &str) -> String {
    let mut name = name.trim().to_ascii_lowercase().replace('-', "_");
    if let Some(suffix) = name.rsplit('.').next() {
        name = suffix.to_string();
    }
    name
}

pub fn gemini_provider_core_tool_is_mutating(name: &str) -> bool {
    let aliases = gemini_provider_core_tool_aliases(name);
    aliases.iter().any(|alias| {
        matches!(
            alias.as_str(),
            "apply_patch"
                | "edit"
                | "replace"
                | "write"
                | "write_file"
                | "exec_command"
                | "run_shell_command"
                | "shell"
                | "bash"
        )
    })
}

pub fn gemini_provider_core_tool_call_command_text(args: &serde_json::Value) -> String {
    if let Some(object) = args.as_object() {
        for key in [
            "command",
            "cmd",
            "shell_command",
            "shellCommand",
            "command_line",
            "commandLine",
            "script",
        ] {
            if let Some(value) = object.get(key).and_then(serde_json::Value::as_str) {
                return value.to_string();
            }
        }
    }
    match args {
        serde_json::Value::String(text) => text.to_string(),
        _ => serde_json::to_string(args).unwrap_or_default(),
    }
}
