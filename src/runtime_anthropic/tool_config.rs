use super::*;

pub(crate) fn runtime_proxy_request_header_value<'a>(
    headers: &'a [(String, String)],
    name: &str,
) -> Option<&'a str> {
    headers
        .iter()
        .find_map(|(header_name, value)| {
            header_name
                .eq_ignore_ascii_case(name)
                .then_some(value.as_str())
        })
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

pub(crate) fn runtime_proxy_anthropic_wants_thinking(value: &serde_json::Value) -> bool {
    value
        .get("thinking")
        .and_then(|thinking| thinking.get("type"))
        .and_then(serde_json::Value::as_str)
        .is_some_and(|thinking| matches!(thinking, "enabled" | "adaptive"))
}

pub(crate) fn runtime_proxy_translate_anthropic_reasoning_effort(
    effort: &str,
    target_model: &str,
) -> Option<&'static str> {
    match effort.trim().to_ascii_lowercase().as_str() {
        "low" => Some("low"),
        "medium" => Some("medium"),
        "high" => Some("high"),
        "max" => Some(
            if runtime_proxy_responses_model_supports_xhigh(target_model) {
                "xhigh"
            } else {
                "high"
            },
        ),
        _ => None,
    }
}

pub(crate) fn runtime_proxy_anthropic_reasoning_effort(
    value: &serde_json::Value,
    target_model: &str,
) -> Option<String> {
    if let Some(effort) = runtime_proxy_claude_reasoning_effort_override() {
        return Some(effort);
    }

    if let Some(effort) = value
        .get("output_config")
        .and_then(|config| config.get("effort"))
        .and_then(serde_json::Value::as_str)
        .and_then(|effort| runtime_proxy_translate_anthropic_reasoning_effort(effort, target_model))
    {
        return Some(effort.to_string());
    }

    let budget_tokens = value
        .get("thinking")
        .and_then(|thinking| thinking.get("budget_tokens"))
        .and_then(serde_json::Value::as_u64)?;
    Some(
        if budget_tokens <= 2_048 {
            "low"
        } else if budget_tokens <= 8_192 {
            "medium"
        } else {
            "high"
        }
        .to_string(),
    )
}

pub(crate) fn runtime_proxy_anthropic_system_instructions(
    value: &serde_json::Value,
) -> Result<Option<String>> {
    let Some(system) = value.get("system") else {
        return Ok(None);
    };
    if let Some(text) = system.as_str() {
        let text = text.trim();
        return Ok((!text.is_empty()).then(|| text.to_string()));
    }
    let blocks = system
        .as_array()
        .context("Anthropic system content must be a string or an array of text blocks")?;
    let text = blocks
        .iter()
        .filter_map(|block| {
            (block.get("type").and_then(serde_json::Value::as_str) == Some("text"))
                .then(|| block.get("text").and_then(serde_json::Value::as_str))
                .flatten()
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    Ok((!text.trim().is_empty()).then_some(text))
}

pub(crate) fn runtime_proxy_anthropic_normalize_tool_schema(
    schema: &serde_json::Value,
) -> serde_json::Value {
    match schema {
        serde_json::Value::Object(map)
            if map.get("type").and_then(serde_json::Value::as_str) == Some("object")
                && !map.contains_key("properties") =>
        {
            let mut normalized = map.clone();
            normalized.insert(
                "properties".to_string(),
                serde_json::Value::Object(serde_json::Map::new()),
            );
            serde_json::Value::Object(normalized)
        }
        _ => schema.clone(),
    }
}

pub(crate) fn runtime_proxy_anthropic_default_tool_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {},
        "additionalProperties": true,
    })
}

pub(crate) const RUNTIME_PROXY_ANTHROPIC_MEMORY_TOOL_INSTRUCTIONS: &str = "\
IMPORTANT: Check the `/memories` directory with the `memory` tool before starting work.
Use the `view` command to recover prior progress and relevant context.
Keep important state in memory as you work because your context window may be interrupted.";

pub(crate) fn runtime_proxy_anthropic_unversioned_tool_type(tool_type: &str) -> String {
    let normalized = tool_type.trim().to_ascii_lowercase();
    let Some((base, suffix)) = normalized.rsplit_once('_') else {
        return normalized;
    };
    if suffix.len() == 8 && suffix.chars().all(|ch| ch.is_ascii_digit()) {
        base.to_string()
    } else {
        normalized
    }
}

pub(crate) fn runtime_proxy_anthropic_builtin_server_tool_name(name: &str) -> Option<&'static str> {
    let normalized = name
        .trim()
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_lowercase();
    match normalized.as_str() {
        "websearch" => Some("web_search"),
        "webfetch" => Some("web_fetch"),
        "codeexecution" => Some("code_execution"),
        "bashcodeexecution" => Some("bash_code_execution"),
        "texteditorcodeexecution" => Some("text_editor_code_execution"),
        "toolsearchtoolregex" => Some("tool_search_tool_regex"),
        "toolsearchtoolbm25" => Some("tool_search_tool_bm25"),
        _ => None,
    }
}

pub(crate) fn runtime_proxy_anthropic_builtin_client_tool_name_from_type(
    tool_type: &str,
) -> Option<&'static str> {
    match runtime_proxy_anthropic_unversioned_tool_type(tool_type).as_str() {
        "bash" => Some("bash"),
        "computer" => Some("computer"),
        "memory" => Some("memory"),
        "text_editor" => Some("str_replace_based_edit_tool"),
        _ => None,
    }
}

pub(crate) fn runtime_proxy_anthropic_tool_version(tool_type: Option<&str>) -> Option<u32> {
    let tool_type = tool_type?;
    let (_, suffix) = tool_type.trim().rsplit_once('_')?;
    if suffix.len() != 8 || !suffix.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    suffix.parse::<u32>().ok()
}

pub(crate) fn runtime_proxy_anthropic_server_tool_name_from_type(
    tool_type: &str,
) -> Option<&'static str> {
    match runtime_proxy_anthropic_unversioned_tool_type(tool_type).as_str() {
        "web_search" => Some("web_search"),
        "web_fetch" => Some("web_fetch"),
        "code_execution" => Some("code_execution"),
        "tool_search_tool_regex" => Some("tool_search_tool_regex"),
        "tool_search_tool_bm25" => Some("tool_search_tool_bm25"),
        _ => None,
    }
}

pub(crate) fn runtime_proxy_anthropic_client_tool_name(name: &str) -> Option<&'static str> {
    let normalized = name.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "bash" => Some("bash"),
        "computer" => Some("computer"),
        "memory" => Some("memory"),
        "str_replace_based_edit_tool" => Some("str_replace_based_edit_tool"),
        _ => runtime_proxy_anthropic_builtin_client_tool_name_from_type(name),
    }
}

pub(crate) fn runtime_proxy_anthropic_builtin_client_tool_description(
    tool: &serde_json::Value,
    tool_type: Option<&str>,
    name: &str,
) -> Option<String> {
    let tool_version = runtime_proxy_anthropic_tool_version(tool_type);
    let canonical_name = runtime_proxy_anthropic_client_tool_name(name).or_else(|| {
        tool_type.and_then(runtime_proxy_anthropic_builtin_client_tool_name_from_type)
    })?;
    match canonical_name {
        "bash" => Some("Run shell commands on the local machine.".to_string()),
        "memory" => Some(
            "Store and retrieve information across conversations using persistent memory files."
                .to_string(),
        ),
        "str_replace_based_edit_tool" => {
            let mut description =
                "Edit local text files with string replacement operations.".to_string();
            if tool_version.is_some_and(|version| version >= 20250728)
                && let Some(max_characters) = tool
                    .get("max_characters")
                    .and_then(serde_json::Value::as_u64)
            {
                description.push(' ');
                description.push_str(&format!(
                    "View results may be truncated to {max_characters} characters."
                ));
            }
            Some(description)
        }
        "computer" => {
            let mut description = "Interact with the graphical computer display.".to_string();
            let mut details = Vec::new();
            if let (Some(width), Some(height)) = (
                tool.get("display_width_px")
                    .and_then(serde_json::Value::as_u64),
                tool.get("display_height_px")
                    .and_then(serde_json::Value::as_u64),
            ) {
                details.push(format!("Display resolution: {width}x{height} pixels."));
            }
            if let Some(display_number) = tool
                .get("display_number")
                .and_then(serde_json::Value::as_u64)
            {
                details.push(format!("Display number: {display_number}."));
            }
            if tool_version.is_some_and(|version| version >= 20251124)
                && tool
                    .get("enable_zoom")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false)
            {
                details.push("Zoom action enabled.".to_string());
            }
            if !details.is_empty() {
                description.push(' ');
                description.push_str(&details.join(" "));
            }
            Some(description)
        }
        _ => None,
    }
}

pub(crate) fn runtime_proxy_anthropic_tool_description(
    tool: &serde_json::Value,
    tool_type: Option<&str>,
    name: &str,
) -> Option<String> {
    let description = tool
        .get("description")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let builtin_description =
        runtime_proxy_anthropic_builtin_client_tool_description(tool, tool_type, name);
    let tool_version = runtime_proxy_anthropic_tool_version(tool_type);
    let canonical_name = runtime_proxy_anthropic_client_tool_name(name)
        .or_else(|| tool_type.and_then(runtime_proxy_anthropic_builtin_client_tool_name_from_type));
    let should_append_builtin = match canonical_name {
        Some("computer") => {
            tool.get("display_width_px")
                .and_then(serde_json::Value::as_u64)
                .is_some()
                || tool
                    .get("display_height_px")
                    .and_then(serde_json::Value::as_u64)
                    .is_some()
                || tool
                    .get("display_number")
                    .and_then(serde_json::Value::as_u64)
                    .is_some()
                || (tool_version.is_some_and(|version| version >= 20251124)
                    && tool
                        .get("enable_zoom")
                        .and_then(serde_json::Value::as_bool)
                        .unwrap_or(false))
        }
        Some("str_replace_based_edit_tool") => {
            tool_version.is_some_and(|version| version >= 20250728)
                && tool
                    .get("max_characters")
                    .and_then(serde_json::Value::as_u64)
                    .is_some()
        }
        _ => false,
    };
    match (description, builtin_description) {
        (Some(description), Some(builtin_description)) if should_append_builtin => {
            Some(format!("{description}\n\n{builtin_description}"))
        }
        (Some(description), None) => Some(description),
        (Some(description), Some(_)) => Some(description),
        (None, Some(builtin_description)) => Some(builtin_description),
        (None, None) => None,
    }
}

pub(crate) fn runtime_proxy_anthropic_builtin_client_tool_schema(
    tool: &serde_json::Value,
    tool_type: Option<&str>,
    name: &str,
) -> Option<serde_json::Value> {
    let tool_version = runtime_proxy_anthropic_tool_version(tool_type);
    let canonical_name = runtime_proxy_anthropic_client_tool_name(name).or_else(|| {
        tool_type.and_then(runtime_proxy_anthropic_builtin_client_tool_name_from_type)
    })?;
    match canonical_name {
        "bash" => Some(serde_json::json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "Shell command to execute."
                },
                "restart": {
                    "type": "boolean",
                    "description": "Restart the shell session before executing the command."
                },
                "timeout_ms": {
                    "type": "integer",
                    "minimum": 1,
                    "description": "Maximum time to wait for the command to finish."
                },
                "max_output_length": {
                    "type": "integer",
                    "minimum": 1,
                    "description": "Maximum number of output characters to return."
                }
            },
            "additionalProperties": true,
        })),
        "str_replace_based_edit_tool" => Some(serde_json::json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "enum": if tool_version.is_some_and(|version| version >= 20250429) {
                        serde_json::json!(["view", "create", "str_replace", "insert"])
                    } else {
                        serde_json::json!(["view", "create", "str_replace", "insert", "undo_edit"])
                    },
                    "description": "Text editor action to perform."
                },
                "path": {
                    "type": "string",
                    "description": "Path of the file to inspect or edit."
                },
                "file_text": {
                    "type": "string",
                    "description": "File contents used when creating a new file."
                },
                "old_str": {
                    "type": "string",
                    "description": "Existing text to replace."
                },
                "new_str": {
                    "type": "string",
                    "description": "Replacement text."
                },
                "insert_line": {
                    "type": "integer",
                    "description": "Line number where new text should be inserted."
                },
                "insert_text": {
                    "type": "string",
                    "description": "Text to insert at the requested line."
                },
                "view_range": {
                    "type": "array",
                    "items": { "type": "integer" },
                    "minItems": 2,
                    "maxItems": 2,
                    "description": "Optional inclusive start and end line numbers for view."
                }
            },
            "additionalProperties": true,
        })),
        "memory" => Some(serde_json::json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "enum": ["view", "create", "str_replace", "insert", "delete", "rename"],
                    "description": "Memory operation to perform."
                },
                "path": {
                    "type": "string",
                    "description": "Path within /memories to inspect or modify."
                },
                "view_range": {
                    "type": "array",
                    "items": { "type": "integer" },
                    "minItems": 2,
                    "maxItems": 2,
                    "description": "Optional inclusive start and end line numbers for view."
                },
                "file_text": {
                    "type": "string",
                    "description": "File contents used when creating a new memory file."
                },
                "old_str": {
                    "type": "string",
                    "description": "Existing text to replace in a memory file."
                },
                "new_str": {
                    "type": "string",
                    "description": "Replacement text for a memory file edit."
                },
                "insert_line": {
                    "type": "integer",
                    "description": "Line number where new text should be inserted."
                },
                "insert_text": {
                    "type": "string",
                    "description": "Text to insert at the requested line."
                },
                "old_path": {
                    "type": "string",
                    "description": "Existing memory path to rename or move."
                },
                "new_path": {
                    "type": "string",
                    "description": "Destination memory path for a rename or move."
                }
            },
            "additionalProperties": true,
        })),
        "computer" => {
            let mut actions = vec!["screenshot", "left_click", "type", "key", "mouse_move"];
            if tool_version.is_some_and(|version| version >= 20250124) {
                actions.extend([
                    "scroll",
                    "left_click_drag",
                    "right_click",
                    "middle_click",
                    "double_click",
                    "triple_click",
                    "left_mouse_down",
                    "left_mouse_up",
                    "hold_key",
                    "wait",
                ]);
            }
            if tool_version.is_some_and(|version| version >= 20251124)
                && tool
                    .get("enable_zoom")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false)
            {
                actions.push("zoom");
            }
            Some(serde_json::json!({
                "type": "object",
                "properties": {
                    "action": {
                        "type": "string",
                        "enum": actions,
                        "description": "Computer action to perform."
                    },
                    "coordinate": {
                        "type": "array",
                        "items": { "type": "integer" },
                        "minItems": 2,
                        "maxItems": 2,
                        "description": "Screen coordinate pair in pixels."
                    },
                    "text": {
                        "type": "string",
                        "description": "Text to type into the active application."
                    },
                    "key": {
                        "type": "string",
                        "description": "Single key or key chord to press."
                    },
                    "keys": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Keys to hold while performing another action."
                    },
                    "scroll_direction": {
                        "type": "string",
                        "enum": ["up", "down", "left", "right"],
                        "description": "Scroll direction."
                    },
                    "scroll_amount": {
                        "type": "integer",
                        "description": "Distance to scroll."
                    },
                    "duration_ms": {
                        "type": "integer",
                        "minimum": 0,
                        "description": "Optional wait duration in milliseconds."
                    },
                    "region": {
                        "type": "array",
                        "items": { "type": "integer" },
                        "minItems": 4,
                        "maxItems": 4,
                        "description": "Optional screen region [x1, y1, x2, y2] for zoom actions."
                    }
                },
                "additionalProperties": true,
            }))
        }
        _ => None,
    }
}

pub(crate) fn runtime_proxy_anthropic_has_ambiguous_native_tool_choice(
    value: &serde_json::Value,
    native_tool_name: &str,
) -> bool {
    let tool_count = value
        .get("tools")
        .and_then(serde_json::Value::as_array)
        .map(Vec::len)
        .unwrap_or(0);
    if tool_count <= 1 {
        return false;
    }
    value
        .get("tool_choice")
        .and_then(|choice| choice.get("type"))
        .and_then(serde_json::Value::as_str)
        .is_some_and(|choice_type| choice_type == "tool")
        && value
            .get("tool_choice")
            .and_then(|choice| choice.get("name"))
            .and_then(serde_json::Value::as_str)
            .and_then(runtime_proxy_anthropic_client_tool_name)
            == Some(native_tool_name)
}

pub(crate) fn runtime_proxy_anthropic_has_ambiguous_native_shell_choice(
    value: &serde_json::Value,
) -> bool {
    runtime_proxy_anthropic_has_ambiguous_native_tool_choice(value, "bash")
}

pub(crate) fn runtime_proxy_anthropic_native_shell_enabled_for_request(
    value: &serde_json::Value,
) -> bool {
    runtime_proxy_claude_native_shell_enabled()
        && !runtime_proxy_anthropic_has_ambiguous_native_shell_choice(value)
}

pub(crate) fn runtime_proxy_anthropic_native_computer_enabled_for_request(
    value: &serde_json::Value,
    target_model: &str,
) -> bool {
    runtime_proxy_claude_native_computer_enabled()
        && runtime_proxy_responses_model_supports_native_computer_tool(target_model)
        && !runtime_proxy_anthropic_has_ambiguous_native_tool_choice(value, "computer")
}

pub(crate) fn runtime_proxy_anthropic_is_tool_use_block_type(block_type: &str) -> bool {
    matches!(block_type, "tool_use" | "server_tool_use" | "mcp_tool_use")
}

pub(crate) fn runtime_proxy_anthropic_is_tool_result_block_type(block_type: &str) -> bool {
    block_type == "tool_result" || block_type.ends_with("_tool_result")
}

pub(crate) fn runtime_proxy_anthropic_is_special_input_item_block_type(block_type: &str) -> bool {
    runtime_proxy_anthropic_is_tool_use_block_type(block_type)
        || runtime_proxy_anthropic_is_tool_result_block_type(block_type)
        || block_type == "mcp_approval_response"
}
