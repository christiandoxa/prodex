use super::*;

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_tool_command_hint(
    object: &serde_json::Map<String, serde_json::Value>,
) -> Option<String> {
    runtime_smart_context_tool_command_hint_from_object(object, 0)
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_tool_command_hint_from_object(
    object: &serde_json::Map<String, serde_json::Value>,
    depth: usize,
) -> Option<String> {
    if depth > SMART_CONTEXT_TOOL_METADATA_SCAN_MAX_DEPTH {
        return None;
    }
    for key in runtime_smart_context_tool_command_keys() {
        if let Some(command) = object
            .get(key)
            .and_then(serde_json::Value::as_str)
            .and_then(runtime_smart_context_bounded_tool_command)
        {
            return Some(command);
        }
    }
    if let Some(command) = object.get("arguments").and_then(|value| {
        runtime_smart_context_tool_command_hint_from_value(value, depth + 1, true)
    }) {
        return Some(command);
    }
    for (field, child) in object {
        if field == "arguments" || runtime_smart_context_tool_output_payload_field(field) {
            continue;
        }
        if let Some(command) =
            runtime_smart_context_tool_command_hint_from_value(child, depth + 1, false)
        {
            return Some(command);
        }
    }
    None
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_tool_command_hint_from_value(
    value: &serde_json::Value,
    depth: usize,
    allow_plain_command: bool,
) -> Option<String> {
    if depth > SMART_CONTEXT_TOOL_METADATA_SCAN_MAX_DEPTH {
        return None;
    }
    match value {
        serde_json::Value::String(text) => {
            if let Some(command) = runtime_smart_context_bounded_tool_command(text)
                && allow_plain_command
                && !text.trim_start().starts_with('{')
            {
                return Some(command);
            }
            serde_json::from_str::<serde_json::Value>(text)
                .ok()
                .and_then(|value| {
                    runtime_smart_context_tool_command_hint_from_value(
                        &value,
                        depth + 1,
                        allow_plain_command,
                    )
                })
        }
        serde_json::Value::Object(object) => {
            runtime_smart_context_tool_command_hint_from_object(object, depth + 1)
        }
        serde_json::Value::Array(items) => items.iter().find_map(|item| {
            runtime_smart_context_tool_command_hint_from_value(item, depth + 1, allow_plain_command)
        }),
        _ => None,
    }
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_tool_command_keys()
-> [&'static str; 4] {
    ["cmd", "command", "shell_command", "shellCommand"]
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_bounded_tool_command(
    text: &str,
) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(
        trimmed
            .chars()
            .take(SMART_CONTEXT_TOOL_COMMAND_HINT_MAX_CHARS)
            .collect(),
    )
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_tool_exit_code_hint(
    object: &serde_json::Map<String, serde_json::Value>,
) -> Option<i32> {
    runtime_smart_context_tool_exit_code_hint_from_object(object, 0)
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_tool_exit_code_hint_from_object(
    object: &serde_json::Map<String, serde_json::Value>,
    depth: usize,
) -> Option<i32> {
    if depth > SMART_CONTEXT_TOOL_METADATA_SCAN_MAX_DEPTH {
        return None;
    }
    for key in runtime_smart_context_tool_exit_code_keys() {
        if let Some(exit_code) = object
            .get(key)
            .and_then(runtime_smart_context_i32_from_json_value)
        {
            return Some(exit_code);
        }
    }
    if let Some(exit_code) = object.get("arguments").and_then(|value| {
        runtime_smart_context_tool_exit_code_hint_from_value(value, depth + 1, true)
    }) {
        return Some(exit_code);
    }
    for (field, child) in object {
        if field == "arguments" || runtime_smart_context_tool_output_payload_field(field) {
            continue;
        }
        if let Some(exit_code) =
            runtime_smart_context_tool_exit_code_hint_from_value(child, depth + 1, false)
        {
            return Some(exit_code);
        }
    }
    None
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_tool_exit_code_hint_from_value(
    value: &serde_json::Value,
    depth: usize,
    allow_plain_exit_code: bool,
) -> Option<i32> {
    if depth > SMART_CONTEXT_TOOL_METADATA_SCAN_MAX_DEPTH {
        return None;
    }
    match value {
        serde_json::Value::String(text) => serde_json::from_str::<serde_json::Value>(text)
            .ok()
            .and_then(|value| {
                runtime_smart_context_tool_exit_code_hint_from_value(
                    &value,
                    depth + 1,
                    allow_plain_exit_code,
                )
            }),
        serde_json::Value::Object(object) => {
            runtime_smart_context_tool_exit_code_hint_from_object(object, depth + 1)
        }
        serde_json::Value::Array(items) => items.iter().find_map(|item| {
            runtime_smart_context_tool_exit_code_hint_from_value(
                item,
                depth + 1,
                allow_plain_exit_code,
            )
        }),
        _ => allow_plain_exit_code
            .then(|| runtime_smart_context_i32_from_json_value(value))
            .flatten(),
    }
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_tool_exit_code_keys()
-> [&'static str; 7] {
    [
        "exit_code",
        "exitCode",
        "exit_status",
        "exitStatus",
        "status_code",
        "statusCode",
        "status",
    ]
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_tool_kind_hint(
    object: &serde_json::Map<String, serde_json::Value>,
    command: Option<&str>,
) -> Option<prodex_context::CommandOutputKind> {
    command
        .and_then(prodex_context::command_output_kind_hint_for_command)
        .or_else(|| runtime_smart_context_tool_kind_hint_from_object(object, 0))
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_tool_kind_hint_from_object(
    object: &serde_json::Map<String, serde_json::Value>,
    depth: usize,
) -> Option<prodex_context::CommandOutputKind> {
    if depth > SMART_CONTEXT_TOOL_METADATA_SCAN_MAX_DEPTH {
        return None;
    }
    for key in runtime_smart_context_tool_kind_keys() {
        if let Some(kind) = object
            .get(key)
            .and_then(serde_json::Value::as_str)
            .and_then(runtime_smart_context_tool_kind_hint_from_text)
        {
            return Some(kind);
        }
    }
    if let Some(kind) = object
        .get("arguments")
        .and_then(|value| runtime_smart_context_tool_kind_hint_from_value(value, depth + 1, true))
    {
        return Some(kind);
    }
    for (field, child) in object {
        if field == "arguments" || runtime_smart_context_tool_output_payload_field(field) {
            continue;
        }
        if let Some(kind) = runtime_smart_context_tool_kind_hint_from_value(child, depth + 1, false)
        {
            return Some(kind);
        }
    }
    None
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_tool_kind_hint_from_value(
    value: &serde_json::Value,
    depth: usize,
    allow_plain_kind: bool,
) -> Option<prodex_context::CommandOutputKind> {
    if depth > SMART_CONTEXT_TOOL_METADATA_SCAN_MAX_DEPTH {
        return None;
    }
    match value {
        serde_json::Value::String(text) => serde_json::from_str::<serde_json::Value>(text)
            .ok()
            .and_then(|value| {
                runtime_smart_context_tool_kind_hint_from_value(&value, depth + 1, allow_plain_kind)
            })
            .or_else(|| {
                allow_plain_kind
                    .then(|| runtime_smart_context_tool_kind_hint_from_text(text))
                    .flatten()
            }),
        serde_json::Value::Object(object) => {
            runtime_smart_context_tool_kind_hint_from_object(object, depth + 1)
        }
        serde_json::Value::Array(items) => items.iter().find_map(|item| {
            runtime_smart_context_tool_kind_hint_from_value(item, depth + 1, allow_plain_kind)
        }),
        _ => None,
    }
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_tool_kind_hint_from_text(
    text: &str,
) -> Option<prodex_context::CommandOutputKind> {
    if text.len() > SMART_CONTEXT_TOOL_COMMAND_HINT_MAX_CHARS {
        return None;
    }
    let normalized = text.trim().to_ascii_lowercase().replace('_', "-");
    match normalized.as_str() {
        "git-status" | "status" => Some(prodex_context::CommandOutputKind::GitStatus),
        "git-diff" | "diff" | "git-show" => Some(prodex_context::CommandOutputKind::GitDiff),
        "rust-diagnostics" | "cargo-test" | "cargo-check" | "cargo-clippy" | "rust" => {
            Some(prodex_context::CommandOutputKind::RustDiagnostics)
        }
        "diagnostics" | "test" | "typecheck" | "type-check" => {
            Some(prodex_context::CommandOutputKind::Diagnostics)
        }
        "git-log" => Some(prodex_context::CommandOutputKind::GitLog),
        "search" | "rg" | "grep" => Some(prodex_context::CommandOutputKind::Search),
        "file-list" | "find" | "tree" => Some(prodex_context::CommandOutputKind::FileList),
        "log-stream" => Some(prodex_context::CommandOutputKind::LogStream),
        "noisy-success" => Some(prodex_context::CommandOutputKind::NoisySuccess),
        "plain" => Some(prodex_context::CommandOutputKind::Plain),
        _ => prodex_context::infer_command_output_kind_from_metadata(&normalized),
    }
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_tool_kind_keys()
-> [&'static str; 8] {
    [
        "kind",
        "output_kind",
        "outputKind",
        "command_kind",
        "commandKind",
        "detected_kind",
        "detectedKind",
        "name",
    ]
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_i32_from_json_value(
    value: &serde_json::Value,
) -> Option<i32> {
    match value {
        serde_json::Value::Number(number) => {
            number.as_i64().and_then(|value| value.try_into().ok())
        }
        serde_json::Value::String(text) => text.trim().parse::<i32>().ok(),
        _ => None,
    }
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_tool_call_id(
    object: &serde_json::Map<String, serde_json::Value>,
) -> Option<&str> {
    ["call_id", "tool_call_id", "id"]
        .into_iter()
        .find_map(|key| object.get(key).and_then(serde_json::Value::as_str))
        .filter(|value| !value.trim().is_empty())
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_tool_output_payload_field(
    key: &str,
) -> bool {
    matches!(key, "output" | "content")
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_artifact_summary(
    artifact: &runtime_proxy_crate::SmartContextArtifactRef,
    compacted: &str,
) -> String {
    let marker = runtime_smart_context_artifact_marker_line("artifact", artifact);
    if compacted.is_empty() {
        return marker;
    }
    format!("{marker}\n{compacted}")
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_repeat_tool_output_reference_summary(
    artifact: &runtime_proxy_crate::SmartContextArtifactRef,
    text: &str,
    call_id: Option<&str>,
) -> String {
    runtime_proxy_crate::smart_context_repeat_tool_output_reference_summary(artifact, text, call_id)
}
