use super::*;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct RuntimeAnthropicServerToolUsage {
    pub(crate) web_search_requests: u64,
    pub(crate) web_fetch_requests: u64,
    pub(crate) code_execution_requests: u64,
    pub(crate) tool_search_requests: u64,
}

impl RuntimeAnthropicServerToolUsage {
    pub(crate) fn add_assign(&mut self, other: Self) {
        self.web_search_requests += other.web_search_requests;
        self.web_fetch_requests += other.web_fetch_requests;
        self.code_execution_requests += other.code_execution_requests;
        self.tool_search_requests += other.tool_search_requests;
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct RuntimeAnthropicServerTools {
    pub(crate) aliases: BTreeMap<String, RuntimeAnthropicRegisteredServerTool>,
    pub(crate) web_search: bool,
    pub(crate) mcp: bool,
    pub(crate) tool_search: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct RuntimeAnthropicRegisteredServerTool {
    pub(crate) response_name: String,
    pub(crate) block_type: String,
}

impl RuntimeAnthropicServerTools {
    pub(crate) fn needs_buffered_translation(&self) -> bool {
        self.web_search
    }

    pub(crate) fn register(&mut self, tool_name: &str, canonical_name: &str) {
        self.register_with_block_type(tool_name, canonical_name, "server_tool_use");
    }

    pub(crate) fn register_with_block_type(
        &mut self,
        tool_name: &str,
        response_name: &str,
        block_type: &str,
    ) {
        let tool_name = tool_name.trim();
        let response_name = response_name.trim();
        let block_type = block_type.trim();
        if tool_name.is_empty() || response_name.is_empty() || block_type.is_empty() {
            return;
        }
        let registration = RuntimeAnthropicRegisteredServerTool {
            response_name: response_name.to_string(),
            block_type: block_type.to_string(),
        };
        self.aliases
            .insert(tool_name.to_string(), registration.clone());
        if let Some(normalized) = runtime_proxy_anthropic_builtin_server_tool_name(tool_name) {
            self.aliases.insert(normalized.to_string(), registration);
        }
        if response_name == "web_search" {
            self.web_search = true;
        } else if block_type == "mcp_tool_use" {
            self.mcp = true;
        } else if response_name.starts_with("tool_search_tool_") {
            self.tool_search = true;
        }
    }

    pub(crate) fn registration_for_call(
        &self,
        tool_name: &str,
    ) -> Option<&RuntimeAnthropicRegisteredServerTool> {
        let tool_name = tool_name.trim();
        if tool_name.is_empty() {
            return None;
        }
        self.aliases.get(tool_name).or_else(|| {
            runtime_proxy_anthropic_builtin_server_tool_name(tool_name)
                .and_then(|normalized| self.aliases.get(normalized))
        })
    }

    pub(crate) fn canonical_name_for_call(&self, tool_name: &str) -> Option<&str> {
        self.registration_for_call(tool_name)
            .map(|registration| registration.response_name.as_str())
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct RuntimeAnthropicMcpServer {
    pub(crate) name: String,
    pub(crate) url: Option<String>,
    pub(crate) authorization_token: Option<String>,
    pub(crate) headers: serde_json::Map<String, serde_json::Value>,
    pub(crate) description: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct RuntimeAnthropicTranslatedTools {
    pub(crate) tools: Vec<serde_json::Value>,
    pub(crate) server_tools: RuntimeAnthropicServerTools,
    pub(crate) tool_name_aliases: BTreeMap<String, String>,
    pub(crate) native_tool_names: BTreeSet<String>,
    pub(crate) memory: bool,
}

impl RuntimeAnthropicTranslatedTools {
    pub(crate) fn implicit_tool_choice(&self) -> Option<serde_json::Value> {
        (self.server_tools.web_search
            && self.tools.len() == 1
            && self.tools.first().and_then(|tool| {
                tool.get("type")
                    .and_then(serde_json::Value::as_str)
                    .map(str::trim)
            }) == Some("web_search"))
        .then(|| serde_json::Value::String("required".to_string()))
    }
}

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

pub(crate) fn runtime_proxy_translate_anthropic_mcp_servers(
    value: &serde_json::Value,
) -> Result<BTreeMap<String, RuntimeAnthropicMcpServer>> {
    let Some(mcp_servers) = value.get("mcp_servers") else {
        return Ok(BTreeMap::new());
    };
    let mcp_servers = mcp_servers
        .as_array()
        .context("Anthropic mcp_servers must be an array when present")?;
    let mut translated = BTreeMap::new();
    for server in mcp_servers {
        let name = server
            .get("name")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .context("Anthropic mcp_server requires a non-empty name")?;
        let mut headers = serde_json::Map::new();
        if let Some(server_headers) = server.get("headers").and_then(serde_json::Value::as_object) {
            for (header_name, header_value) in server_headers {
                let Some(header_value) = header_value
                    .as_str()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                else {
                    continue;
                };
                headers.insert(
                    header_name.clone(),
                    serde_json::Value::String(header_value.to_string()),
                );
            }
        }
        translated.insert(
            name.to_string(),
            RuntimeAnthropicMcpServer {
                name: name.to_string(),
                url: server
                    .get("url")
                    .and_then(serde_json::Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string),
                authorization_token: server
                    .get("authorization_token")
                    .and_then(serde_json::Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string),
                headers,
                description: server
                    .get("description")
                    .and_then(serde_json::Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string),
            },
        );
    }
    Ok(translated)
}

pub(crate) fn runtime_proxy_translate_anthropic_mcp_tool(
    tool: &serde_json::Value,
    mcp_servers: &BTreeMap<String, RuntimeAnthropicMcpServer>,
) -> Result<Option<serde_json::Value>> {
    let Some(server_name) = tool
        .get("mcp_server_name")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(None);
    };
    let Some(server) = mcp_servers.get(server_name) else {
        return Ok(None);
    };
    let Some(server_url) = server.url.as_deref() else {
        return Ok(None);
    };

    let default_config = tool
        .get("default_config")
        .and_then(serde_json::Value::as_object);
    let default_enabled = default_config
        .and_then(|config| config.get("enabled"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(true);
    let default_defer_loading = default_config
        .and_then(|config| config.get("defer_loading"))
        .and_then(serde_json::Value::as_bool);

    let mut allowlisted_tools = Vec::new();
    let mut has_unrepresentable_denylist = false;
    if let Some(configs) = tool.get("configs") {
        let Some(configs) = configs.as_object() else {
            return Ok(None);
        };
        for (tool_name, config) in configs {
            let Some(config) = config.as_object() else {
                return Ok(None);
            };
            let enabled = config
                .get("enabled")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(default_enabled);
            if default_enabled {
                if !enabled {
                    has_unrepresentable_denylist = true;
                    break;
                }
            } else if enabled {
                allowlisted_tools.push(serde_json::Value::String(tool_name.clone()));
            }
        }
    }
    if has_unrepresentable_denylist {
        return Ok(None);
    }

    let mut translated = serde_json::Map::new();
    translated.insert(
        "type".to_string(),
        serde_json::Value::String("mcp".to_string()),
    );
    translated.insert(
        "server_label".to_string(),
        serde_json::Value::String(server.name.clone()),
    );
    translated.insert(
        "server_url".to_string(),
        serde_json::Value::String(server_url.to_string()),
    );
    translated.insert(
        "require_approval".to_string(),
        serde_json::Value::String("never".to_string()),
    );
    if !default_enabled {
        translated.insert(
            "allowed_tools".to_string(),
            serde_json::Value::Array(allowlisted_tools),
        );
    }
    if let Some(defer_loading) = default_defer_loading {
        translated.insert(
            "defer_loading".to_string(),
            serde_json::Value::Bool(defer_loading),
        );
    }
    if let Some(authorization) = server.authorization_token.as_deref() {
        translated.insert(
            "authorization".to_string(),
            serde_json::Value::String(authorization.to_string()),
        );
    }
    if !server.headers.is_empty() {
        translated.insert(
            "headers".to_string(),
            serde_json::Value::Object(server.headers.clone()),
        );
    }
    if let Some(server_description) = tool
        .get("description")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .or(server.description.as_deref())
    {
        translated.insert(
            "server_description".to_string(),
            serde_json::Value::String(server_description.to_string()),
        );
    }
    Ok(Some(serde_json::Value::Object(translated)))
}

pub(crate) fn runtime_proxy_translate_anthropic_tools(
    value: &serde_json::Value,
    native_shell_enabled: bool,
    native_computer_enabled: bool,
) -> Result<RuntimeAnthropicTranslatedTools> {
    let Some(tools) = value.get("tools") else {
        return Ok(RuntimeAnthropicTranslatedTools::default());
    };
    let tools = tools
        .as_array()
        .context("Anthropic tools must be an array when present")?;
    let mcp_servers = runtime_proxy_translate_anthropic_mcp_servers(value)?;
    let mut translated = RuntimeAnthropicTranslatedTools::default();
    for tool in tools {
        let tool_type = tool
            .get("type")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let (tool_value, tool_state) = runtime_proxy_translate_anthropic_tool(
            tool,
            &mcp_servers,
            native_shell_enabled,
            native_computer_enabled,
        )?;
        let translated_name = tool_value
            .get("name")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .or_else(|| {
                (tool_value.get("type").and_then(serde_json::Value::as_str) == Some("shell"))
                    .then_some("bash")
                    .or_else(|| {
                        (tool_value.get("type").and_then(serde_json::Value::as_str)
                            == Some("computer"))
                        .then_some("computer")
                    })
            });
        if let Some(translated_name) = translated_name {
            if let Some(original_name) = tool
                .get("name")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                translated
                    .tool_name_aliases
                    .insert(original_name.to_string(), translated_name.to_string());
            }
            if let Some(tool_type) = tool
                .get("type")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                translated
                    .tool_name_aliases
                    .insert(tool_type.to_string(), translated_name.to_string());
            }
            if matches!(
                tool_value.get("type").and_then(serde_json::Value::as_str),
                Some("shell" | "computer")
            ) {
                translated
                    .native_tool_names
                    .insert(translated_name.to_string());
            }
        }
        translated.memory |= tool_type
            .is_some_and(|value| runtime_proxy_anthropic_unversioned_tool_type(value) == "memory");
        let RuntimeAnthropicServerTools {
            aliases,
            web_search,
            mcp,
            tool_search,
        } = tool_state;
        translated.tools.push(tool_value);
        translated.server_tools.aliases.extend(aliases);
        translated.server_tools.web_search |= web_search;
        translated.server_tools.mcp |= mcp;
        translated.server_tools.tool_search |= tool_search;
    }
    Ok(translated)
}

pub(crate) fn runtime_proxy_anthropic_append_tool_instructions(
    instructions: Option<String>,
    has_memory_tool: bool,
) -> Option<String> {
    if !has_memory_tool {
        return instructions;
    }

    Some(match instructions {
        Some(instructions) if !instructions.trim().is_empty() => {
            format!("{instructions}\n\n{RUNTIME_PROXY_ANTHROPIC_MEMORY_TOOL_INSTRUCTIONS}")
        }
        _ => RUNTIME_PROXY_ANTHROPIC_MEMORY_TOOL_INSTRUCTIONS.to_string(),
    })
}

pub(crate) fn runtime_proxy_translate_anthropic_tool(
    tool: &serde_json::Value,
    mcp_servers: &BTreeMap<String, RuntimeAnthropicMcpServer>,
    native_shell_enabled: bool,
    native_computer_enabled: bool,
) -> Result<(serde_json::Value, RuntimeAnthropicServerTools)> {
    let mut server_tools = RuntimeAnthropicServerTools::default();
    let tool_type = tool
        .get("type")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let tool_name = tool
        .get("name")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let is_claude_named_generic_builtin_tool = tool_type.is_none()
        && tool.get("input_schema").is_some()
        && matches!(
            tool_name.and_then(runtime_proxy_anthropic_builtin_server_tool_name),
            Some("web_search" | "web_fetch")
        );
    if tool_type
        .is_some_and(|value| runtime_proxy_anthropic_unversioned_tool_type(value) == "mcp_toolset")
        && let Some(translated) = runtime_proxy_translate_anthropic_mcp_tool(tool, mcp_servers)?
    {
        server_tools.mcp = true;
        return Ok((translated, server_tools));
    }
    if let Some(canonical_name) = tool_type
        .and_then(runtime_proxy_anthropic_server_tool_name_from_type)
        .or_else(|| {
            (!is_claude_named_generic_builtin_tool)
                .then_some(())
                .and(tool_name.and_then(runtime_proxy_anthropic_builtin_server_tool_name))
        })
    {
        let tool_name = tool_name.unwrap_or(canonical_name);
        server_tools.register(tool_name, canonical_name);
        if canonical_name == "web_search" {
            let mut translated = serde_json::Map::new();
            translated.insert(
                "type".to_string(),
                serde_json::Value::String("web_search".to_string()),
            );
            let allowed_domains = tool
                .get("allowed_domains")
                .and_then(serde_json::Value::as_array)
                .map(|domains| {
                    domains
                        .iter()
                        .filter_map(|domain| {
                            domain
                                .as_str()
                                .map(str::trim)
                                .filter(|value| !value.is_empty())
                                .map(|value| serde_json::Value::String(value.to_string()))
                        })
                        .collect::<Vec<_>>()
                })
                .filter(|domains| !domains.is_empty());
            if let Some(allowed_domains) = allowed_domains {
                translated.insert(
                    "filters".to_string(),
                    serde_json::json!({
                        "allowed_domains": allowed_domains,
                    }),
                );
            }
            if let Some(user_location) = tool
                .get("user_location")
                .filter(|value| value.is_object())
                .cloned()
            {
                translated.insert("user_location".to_string(), user_location);
            }
            return Ok((serde_json::Value::Object(translated), server_tools));
        }

        let mut translated = serde_json::Map::new();
        translated.insert(
            "type".to_string(),
            serde_json::Value::String("function".to_string()),
        );
        translated.insert(
            "name".to_string(),
            serde_json::Value::String(tool_name.to_string()),
        );
        if let Some(description) = tool
            .get("description")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            translated.insert(
                "description".to_string(),
                serde_json::Value::String(description.to_string()),
            );
        }
        if let Some(schema) = tool.get("input_schema") {
            translated.insert(
                "parameters".to_string(),
                runtime_proxy_anthropic_normalize_tool_schema(schema),
            );
        } else {
            translated.insert(
                "parameters".to_string(),
                runtime_proxy_anthropic_default_tool_schema(),
            );
        }
        return Ok((serde_json::Value::Object(translated), server_tools));
    }

    if native_shell_enabled
        && tool_type
            .is_some_and(|value| runtime_proxy_anthropic_unversioned_tool_type(value) == "bash")
    {
        return Ok((serde_json::json!({ "type": "shell" }), server_tools));
    }
    if native_computer_enabled
        && tool_type
            .is_some_and(|value| runtime_proxy_anthropic_unversioned_tool_type(value) == "computer")
    {
        return Ok((serde_json::json!({ "type": "computer" }), server_tools));
    }

    let name = tool
        .get("name")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .or_else(|| tool_type.and_then(runtime_proxy_anthropic_builtin_client_tool_name_from_type))
        .or(tool_type)
        .context("Anthropic tool definition is missing a non-empty name")?;
    let mut translated = serde_json::Map::new();
    translated.insert(
        "type".to_string(),
        serde_json::Value::String("function".to_string()),
    );
    translated.insert(
        "name".to_string(),
        serde_json::Value::String(name.to_string()),
    );
    if let Some(description) = runtime_proxy_anthropic_tool_description(tool, tool_type, name) {
        translated.insert(
            "description".to_string(),
            serde_json::Value::String(description),
        );
    }
    if let Some(schema) = tool.get("input_schema") {
        translated.insert(
            "parameters".to_string(),
            runtime_proxy_anthropic_normalize_tool_schema(schema),
        );
    } else if let Some(schema) =
        runtime_proxy_anthropic_builtin_client_tool_schema(tool, tool_type, name)
    {
        translated.insert("parameters".to_string(), schema);
    } else if tool_type.is_some() {
        translated.insert(
            "parameters".to_string(),
            runtime_proxy_anthropic_default_tool_schema(),
        );
    }
    Ok((serde_json::Value::Object(translated), server_tools))
}

pub(crate) fn runtime_proxy_translate_anthropic_tool_choice(
    value: &serde_json::Value,
    server_tools: &RuntimeAnthropicServerTools,
    tool_name_aliases: &BTreeMap<String, String>,
    native_tool_names: &BTreeSet<String>,
) -> Result<Option<serde_json::Value>> {
    let Some(choice) = value.get("tool_choice") else {
        return Ok(None);
    };
    let choice_type = choice
        .get("type")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .context("Anthropic tool_choice requires a non-empty type")?;
    Ok(match choice_type {
        "auto" => Some(serde_json::Value::String("auto".to_string())),
        "any" => Some(serde_json::Value::String("required".to_string())),
        "none" => Some(serde_json::Value::String("none".to_string())),
        "tool" => {
            let name = choice
                .get("name")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .context("Anthropic tool_choice type=tool requires a non-empty name")?;
            let name = tool_name_aliases
                .get(name)
                .map(String::as_str)
                .unwrap_or(name);
            if native_tool_names.contains(name) {
                return Ok(Some(serde_json::Value::String("required".to_string())));
            }
            if server_tools.web_search
                && server_tools.canonical_name_for_call(name) == Some("web_search")
            {
                return Ok(Some(serde_json::Value::String("required".to_string())));
            }
            Some(serde_json::json!({
                "type": "function",
                "name": name,
            }))
        }
        other => bail!("Unsupported Anthropic tool_choice type '{other}'"),
    })
}
