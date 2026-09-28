//! Exact-output command matching against assistant tool calls.

use prodex_mojo_core::gemini_guardrails::{
    gemini_command_matches_required, gemini_normalized_command_is_empty,
};

pub(in crate::gemini_bridge::tooling::guardrails::exact_output) fn gemini_provider_core_tool_command_matches_required(
    messages_before_tool: &[serde_json::Value],
    required_command: &str,
) -> bool {
    if gemini_normalized_command_is_empty(required_command)
        .expect("Mojo Gemini command normalization should accept Rust strings")
    {
        return true;
    }
    messages_before_tool
        .iter()
        .rev()
        .filter(|message| {
            message.get("role").and_then(serde_json::Value::as_str) == Some("assistant")
        })
        .flat_map(gemini_provider_core_assistant_tool_commands)
        .any(|command| {
            gemini_command_matches_required(required_command, &command)
                .expect("Mojo Gemini command matching should accept Rust strings")
        })
}

fn gemini_provider_core_assistant_tool_commands(message: &serde_json::Value) -> Vec<String> {
    let mut commands = Vec::new();
    if let Some(tool_calls) = message
        .get("tool_calls")
        .and_then(serde_json::Value::as_array)
    {
        for tool_call in tool_calls {
            if let Some(function) = tool_call.get("function") {
                gemini_provider_core_collect_command_from_function(function, &mut commands);
            }
        }
    }
    gemini_provider_core_collect_command_from_function(message, &mut commands);
    commands
}

fn gemini_provider_core_collect_command_from_function(
    function: &serde_json::Value,
    commands: &mut Vec<String>,
) {
    let arguments = function
        .get("arguments")
        .or_else(|| function.get("args"))
        .or_else(|| function.get("input"));
    let Some(arguments) = arguments else {
        return;
    };
    let parsed = match arguments {
        serde_json::Value::String(text) => {
            serde_json::from_str::<serde_json::Value>(text).unwrap_or_else(|_| arguments.clone())
        }
        _ => arguments.clone(),
    };
    gemini_provider_core_collect_command_strings(&parsed, commands);
}

fn gemini_provider_core_collect_command_strings(
    value: &serde_json::Value,
    commands: &mut Vec<String>,
) {
    let Some(object) = value.as_object() else {
        return;
    };
    for key in ["cmd", "command", "shell_command"] {
        if let Some(command) = object.get(key).and_then(serde_json::Value::as_str)
            && !command.trim().is_empty()
        {
            commands.push(command.trim().to_string());
        }
    }
}
