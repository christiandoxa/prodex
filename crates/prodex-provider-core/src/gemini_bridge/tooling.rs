//! Gemini tool declaration and tool-output guard helpers.

use prodex_mojo_core::gemini_tooling_policy::gemini_tool_name_policy;
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
    let policy = gemini_tool_name_policy(name)
        .expect("Mojo Gemini tool-name policy should accept Rust strings");
    let mut aliases = BTreeSet::new();
    aliases.insert(policy.normalized);
    aliases.insert(policy.suffix);
    aliases.extend(policy.aliases.into_iter().map(str::to_string));
    aliases
}

pub fn gemini_provider_core_canonical_output_tool_name(name: &str) -> String {
    let policy = gemini_tool_name_policy(name)
        .expect("Mojo Gemini tool-name policy should accept Rust strings");
    if policy.canonical_exec {
        "exec_command".to_string()
    } else {
        name.to_string()
    }
}

pub fn gemini_provider_core_normalize_tool_name(name: &str) -> String {
    gemini_tool_name_policy(name)
        .expect("Mojo Gemini tool-name policy should accept Rust strings")
        .normalized
}

pub fn gemini_provider_core_tool_is_mutating(name: &str) -> bool {
    gemini_tool_name_policy(name)
        .expect("Mojo Gemini tool-name policy should accept Rust strings")
        .mutating
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gemini_tooling_policy_is_mojo_authoritative() {
        assert_eq!(
            gemini_provider_core_normalize_tool_name(" Namespace.RUN-SHELL-COMMAND "),
            "run_shell_command"
        );
        let aliases = gemini_provider_core_tool_aliases("mcp__repo__read-file");
        assert!(aliases.contains("mcp__repo__read_file"));
        assert!(aliases.contains("read_file"));
        assert!(!aliases.contains("read"));
        assert!(gemini_provider_core_tool_is_mutating(
            "mcp__repo__exec-command"
        ));
        assert!(!gemini_provider_core_tool_is_mutating(
            "mcp__repo__read-file"
        ));
        assert_eq!(
            gemini_provider_core_canonical_output_tool_name("RUN-SHELL-COMMAND"),
            "exec_command"
        );
        assert_eq!(
            gemini_provider_core_canonical_output_tool_name("shell"),
            "shell"
        );
    }
}
