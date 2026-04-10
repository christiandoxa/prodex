use super::*;

pub(super) fn handle_runtime_proxy_anthropic_compat_request(
    request: &tiny_http::Request,
) -> Option<tiny_http::ResponseBox> {
    let path = path_without_query(request.url());
    let method = request.method().as_str();
    if !method.eq_ignore_ascii_case("GET") && !method.eq_ignore_ascii_case("HEAD") {
        return None;
    }

    match path {
        "/" => Some(build_runtime_proxy_json_response(
            200,
            serde_json::json!({
                "service": "prodex",
                "status": "ok",
                "version": env!("CARGO_PKG_VERSION"),
            })
            .to_string(),
        )),
        RUNTIME_PROXY_ANTHROPIC_HEALTH_PATH => Some(build_runtime_proxy_json_response(
            200,
            serde_json::json!({
                "status": "ok",
            })
            .to_string(),
        )),
        RUNTIME_PROXY_ANTHROPIC_MODELS_PATH => Some(build_runtime_proxy_json_response(
            200,
            runtime_proxy_anthropic_models_list().to_string(),
        )),
        _ => runtime_proxy_anthropic_model_id_from_path(path).map(|model_id| {
            build_runtime_proxy_json_response(
                200,
                runtime_proxy_anthropic_model_descriptor(model_id).to_string(),
            )
        }),
    }
}

pub(super) fn runtime_proxy_anthropic_models_list() -> serde_json::Value {
    let data = runtime_proxy_responses_model_descriptors()
        .iter()
        .map(|descriptor| runtime_proxy_anthropic_model_descriptor(descriptor.id))
        .collect::<Vec<_>>();
    let first_id = data
        .first()
        .and_then(|model| model.get("id"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    let last_id = data
        .last()
        .and_then(|model| model.get("id"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    serde_json::json!({
        "data": data,
        "first_id": first_id,
        "has_more": false,
        "last_id": last_id,
    })
}

pub(super) fn runtime_proxy_anthropic_model_descriptor(model_id: &str) -> serde_json::Value {
    let supported_effort_levels = runtime_proxy_responses_model_supported_effort_levels(model_id);
    serde_json::json!({
        "type": "model",
        "id": model_id,
        "display_name": runtime_proxy_anthropic_model_display_name(model_id),
        "created_at": RUNTIME_PROXY_ANTHROPIC_MODEL_CREATED_AT,
        "supportsEffort": true,
        "supportedEffortLevels": supported_effort_levels,
    })
}

pub(super) fn runtime_proxy_anthropic_model_display_name(model_id: &str) -> String {
    runtime_proxy_responses_model_descriptor(model_id)
        .map(|descriptor| descriptor.display_name.to_string())
        .unwrap_or_else(|| model_id.to_string())
}

pub(super) fn runtime_proxy_anthropic_model_id_from_path(path: &str) -> Option<&str> {
    path.strip_prefix(&format!("{RUNTIME_PROXY_ANTHROPIC_MODELS_PATH}/"))
        .filter(|model_id| !model_id.is_empty())
}

#[derive(Debug, Clone)]
pub(super) struct RuntimeAnthropicMessagesRequest {
    pub(super) translated_request: RuntimeProxyRequest,
    pub(super) requested_model: String,
    pub(super) stream: bool,
    pub(super) want_thinking: bool,
    pub(super) server_tools: RuntimeAnthropicServerTools,
    pub(super) carried_web_search_requests: u64,
    pub(super) carried_web_fetch_requests: u64,
    pub(super) carried_code_execution_requests: u64,
    pub(super) carried_tool_search_requests: u64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct RuntimeAnthropicServerToolUsage {
    pub(super) web_search_requests: u64,
    pub(super) web_fetch_requests: u64,
    pub(super) code_execution_requests: u64,
    pub(super) tool_search_requests: u64,
}

impl RuntimeAnthropicServerToolUsage {
    fn add_assign(&mut self, other: Self) {
        self.web_search_requests += other.web_search_requests;
        self.web_fetch_requests += other.web_fetch_requests;
        self.code_execution_requests += other.code_execution_requests;
        self.tool_search_requests += other.tool_search_requests;
    }
}

#[derive(Debug, Clone, Default)]
pub(super) struct RuntimeAnthropicServerTools {
    aliases: BTreeMap<String, RuntimeAnthropicRegisteredServerTool>,
    web_search: bool,
    mcp: bool,
    tool_search: bool,
}

#[derive(Debug, Clone)]
pub(super) struct RuntimeAnthropicRegisteredServerTool {
    response_name: String,
    block_type: String,
}

impl RuntimeAnthropicServerTools {
    pub(super) fn needs_buffered_translation(&self) -> bool {
        self.web_search || self.mcp
    }

    pub(super) fn register(&mut self, tool_name: &str, canonical_name: &str) {
        self.register_with_block_type(tool_name, canonical_name, "server_tool_use");
    }

    fn register_with_block_type(&mut self, tool_name: &str, response_name: &str, block_type: &str) {
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

    fn registration_for_call(
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

    fn canonical_name_for_call(&self, tool_name: &str) -> Option<&str> {
        self.registration_for_call(tool_name)
            .map(|registration| registration.response_name.as_str())
    }
}

#[derive(Debug, Clone, Default)]
pub(super) struct RuntimeAnthropicMcpServer {
    name: String,
    url: Option<String>,
    authorization_token: Option<String>,
    headers: serde_json::Map<String, serde_json::Value>,
    description: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub(super) struct RuntimeAnthropicTranslatedTools {
    tools: Vec<serde_json::Value>,
    server_tools: RuntimeAnthropicServerTools,
    tool_name_aliases: BTreeMap<String, String>,
    native_tool_names: BTreeSet<String>,
    memory: bool,
}

impl RuntimeAnthropicTranslatedTools {
    fn implicit_tool_choice(&self) -> Option<serde_json::Value> {
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

pub(super) fn runtime_proxy_request_header_value<'a>(
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

pub(super) fn runtime_proxy_anthropic_wants_thinking(value: &serde_json::Value) -> bool {
    value
        .get("thinking")
        .and_then(|thinking| thinking.get("type"))
        .and_then(serde_json::Value::as_str)
        .is_some_and(|thinking| matches!(thinking, "enabled" | "adaptive"))
}

pub(super) fn runtime_proxy_translate_anthropic_reasoning_effort(
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

pub(super) fn runtime_proxy_anthropic_reasoning_effort(
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

pub(super) fn runtime_proxy_anthropic_system_instructions(
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

pub(super) fn runtime_proxy_anthropic_normalize_tool_schema(
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

pub(super) fn runtime_proxy_anthropic_default_tool_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {},
        "additionalProperties": true,
    })
}

pub(super) const RUNTIME_PROXY_ANTHROPIC_MEMORY_TOOL_INSTRUCTIONS: &str = "\
IMPORTANT: Check the `/memories` directory with the `memory` tool before starting work.
Use the `view` command to recover prior progress and relevant context.
Keep important state in memory as you work because your context window may be interrupted.";

pub(super) fn runtime_proxy_anthropic_unversioned_tool_type(tool_type: &str) -> String {
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

pub(super) fn runtime_proxy_anthropic_builtin_server_tool_name(name: &str) -> Option<&'static str> {
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

pub(super) fn runtime_proxy_anthropic_builtin_client_tool_name_from_type(
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

pub(super) fn runtime_proxy_anthropic_tool_version(tool_type: Option<&str>) -> Option<u32> {
    let tool_type = tool_type?;
    let (_, suffix) = tool_type.trim().rsplit_once('_')?;
    if suffix.len() != 8 || !suffix.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    suffix.parse::<u32>().ok()
}

pub(super) fn runtime_proxy_anthropic_server_tool_name_from_type(
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

pub(super) fn runtime_proxy_anthropic_client_tool_name(name: &str) -> Option<&'static str> {
    let normalized = name.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "bash" => Some("bash"),
        "computer" => Some("computer"),
        "memory" => Some("memory"),
        "str_replace_based_edit_tool" => Some("str_replace_based_edit_tool"),
        _ => runtime_proxy_anthropic_builtin_client_tool_name_from_type(name),
    }
}

pub(super) fn runtime_proxy_anthropic_builtin_client_tool_description(
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

pub(super) fn runtime_proxy_anthropic_tool_description(
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

pub(super) fn runtime_proxy_anthropic_builtin_client_tool_schema(
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

pub(super) fn runtime_proxy_anthropic_has_ambiguous_native_tool_choice(
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

pub(super) fn runtime_proxy_anthropic_has_ambiguous_native_shell_choice(
    value: &serde_json::Value,
) -> bool {
    runtime_proxy_anthropic_has_ambiguous_native_tool_choice(value, "bash")
}

pub(super) fn runtime_proxy_anthropic_native_shell_enabled_for_request(
    value: &serde_json::Value,
) -> bool {
    runtime_proxy_claude_native_shell_enabled()
        && !runtime_proxy_anthropic_has_ambiguous_native_shell_choice(value)
}

pub(super) fn runtime_proxy_anthropic_native_computer_enabled_for_request(
    value: &serde_json::Value,
    target_model: &str,
) -> bool {
    runtime_proxy_claude_native_computer_enabled()
        && runtime_proxy_responses_model_supports_native_computer_tool(target_model)
        && !runtime_proxy_anthropic_has_ambiguous_native_tool_choice(value, "computer")
}

pub(super) fn runtime_proxy_anthropic_is_tool_use_block_type(block_type: &str) -> bool {
    matches!(block_type, "tool_use" | "server_tool_use" | "mcp_tool_use")
}

pub(super) fn runtime_proxy_anthropic_is_tool_result_block_type(block_type: &str) -> bool {
    block_type == "tool_result" || block_type.ends_with("_tool_result")
}

pub(super) fn runtime_proxy_anthropic_is_special_input_item_block_type(block_type: &str) -> bool {
    runtime_proxy_anthropic_is_tool_use_block_type(block_type)
        || runtime_proxy_anthropic_is_tool_result_block_type(block_type)
        || block_type == "mcp_approval_response"
}

pub(super) fn runtime_proxy_translate_anthropic_mcp_servers(
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

pub(super) fn runtime_proxy_translate_anthropic_mcp_tool(
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

pub(super) fn runtime_proxy_translate_anthropic_tools(
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

pub(super) fn runtime_proxy_anthropic_append_tool_instructions(
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

pub(super) fn runtime_proxy_translate_anthropic_tool(
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
    if tool_type
        .is_some_and(|value| runtime_proxy_anthropic_unversioned_tool_type(value) == "mcp_toolset")
        && let Some(translated) = runtime_proxy_translate_anthropic_mcp_tool(tool, mcp_servers)?
    {
        server_tools.mcp = true;
        return Ok((translated, server_tools));
    }
    if let Some(canonical_name) = tool_type
        .and_then(runtime_proxy_anthropic_server_tool_name_from_type)
        .or_else(|| tool_name.and_then(runtime_proxy_anthropic_builtin_server_tool_name))
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

pub(super) fn runtime_proxy_translate_anthropic_tool_choice(
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

pub(super) fn runtime_proxy_anthropic_image_data_url(block: &serde_json::Value) -> Option<String> {
    let source = block.get("source")?;
    if source.get("type").and_then(serde_json::Value::as_str) != Some("base64") {
        return None;
    }
    let media_type = source
        .get("media_type")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    let data = source
        .get("data")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    Some(format!("data:{media_type};base64,{data}"))
}

pub(super) fn runtime_proxy_translate_anthropic_image_part(
    block: &serde_json::Value,
) -> Option<serde_json::Value> {
    let image_url = runtime_proxy_anthropic_image_data_url(block)?;
    Some(serde_json::json!({
        "type": "input_image",
        "image_url": image_url,
    }))
}

pub(super) fn runtime_proxy_translate_anthropic_document_text(
    block: &serde_json::Value,
) -> Option<String> {
    let source = block.get("source")?;
    match source.get("type").and_then(serde_json::Value::as_str) {
        Some("text") => source
            .get("text")
            .or_else(|| source.get("data"))
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
        Some("base64") => {
            let media_type = source
                .get("media_type")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .unwrap_or_default()
                .to_ascii_lowercase();
            if !(media_type.starts_with("text/") || media_type == "application/json") {
                return None;
            }
            let data = source
                .get("data")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())?;
            let decoded = base64::engine::general_purpose::STANDARD
                .decode(data.as_bytes())
                .ok()?;
            String::from_utf8(decoded)
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        }
        _ => None,
    }
}

pub(super) fn runtime_proxy_translate_anthropic_text_from_block(
    block: &serde_json::Value,
) -> Option<String> {
    match block.get("type").and_then(serde_json::Value::as_str) {
        Some("text") => block
            .get("text")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
        Some("document") => runtime_proxy_translate_anthropic_document_text(block),
        Some("web_fetch_result") => block
            .get("content")
            .and_then(runtime_proxy_translate_anthropic_text_from_block),
        Some("code_execution_result" | "bash_code_execution_result") => {
            let stdout = block
                .get("stdout")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty());
            let stderr = block
                .get("stderr")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty());
            match (stdout, stderr) {
                (Some(stdout), Some(stderr)) => Some(format!("{stdout}\n{stderr}")),
                (Some(stdout), None) => Some(stdout.to_string()),
                (None, Some(stderr)) => Some(stderr.to_string()),
                (None, None) => None,
            }
        }
        Some("text_editor_code_execution_result") => block
            .get("content")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .or_else(|| {
                block
                    .get("lines")
                    .and_then(serde_json::Value::as_array)
                    .map(|lines| {
                        lines
                            .iter()
                            .filter_map(serde_json::Value::as_str)
                            .map(str::trim)
                            .filter(|value| !value.is_empty())
                            .collect::<Vec<_>>()
                            .join("\n")
                    })
            })
            .filter(|value| !value.is_empty()),
        Some(result_type) if result_type.ends_with("_tool_result_error") => block
            .get("error_code")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| format!("Error: {value}")),
        _ => None,
    }
}

pub(super) fn runtime_proxy_translate_anthropic_block_fallback_text(
    block: &serde_json::Value,
) -> Option<String> {
    let block_type = block
        .get("type")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    if block_type == "image"
        || runtime_proxy_anthropic_is_tool_use_block_type(block_type)
        || runtime_proxy_anthropic_is_tool_result_block_type(block_type)
    {
        return None;
    }
    let serialized = serde_json::to_string(block).ok()?;
    Some(format!("[anthropic:{block_type}] {serialized}"))
}

pub(super) fn runtime_proxy_translate_anthropic_mcp_approval_response(
    block: &serde_json::Value,
) -> Result<serde_json::Value> {
    let approval_request_id = block
        .get("approval_request_id")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .context(
            "Anthropic mcp_approval_response block requires a non-empty approval_request_id",
        )?;
    let approve = block
        .get("approve")
        .and_then(serde_json::Value::as_bool)
        .context("Anthropic mcp_approval_response block requires approve=true/false")?;

    let mut translated = serde_json::Map::new();
    translated.insert(
        "type".to_string(),
        serde_json::Value::String("mcp_approval_response".to_string()),
    );
    translated.insert(
        "approval_request_id".to_string(),
        serde_json::Value::String(approval_request_id.to_string()),
    );
    translated.insert("approve".to_string(), serde_json::Value::Bool(approve));
    if let Some(id) = block
        .get("id")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        translated.insert("id".to_string(), serde_json::Value::String(id.to_string()));
    }
    if let Some(reason) = block
        .get("reason")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        translated.insert(
            "reason".to_string(),
            serde_json::Value::String(reason.to_string()),
        );
    }
    Ok(serde_json::Value::Object(translated))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RuntimeAnthropicNativeClientToolKind {
    Shell,
    Computer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RuntimeAnthropicNativeClientToolCall {
    kind: RuntimeAnthropicNativeClientToolKind,
    max_output_length: Option<u64>,
}

pub(super) fn runtime_proxy_translate_anthropic_shell_tool_call(
    block: &serde_json::Value,
) -> Option<(
    String,
    serde_json::Value,
    RuntimeAnthropicNativeClientToolCall,
)> {
    let name = block
        .get("name")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    if runtime_proxy_anthropic_client_tool_name(name) != Some("bash") {
        return None;
    }
    let call_id = block
        .get("id")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())?
        .to_string();
    let input = block.get("input")?.as_object()?;
    if input
        .get("restart")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
    {
        return None;
    }
    let command = input
        .get("command")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    let mut action = serde_json::Map::new();
    action.insert(
        "commands".to_string(),
        serde_json::json!([command.to_string()]),
    );
    if let Some(timeout) = input.get("timeout_ms").and_then(serde_json::Value::as_u64) {
        action.insert(
            "timeout_ms".to_string(),
            serde_json::Value::Number(timeout.into()),
        );
    }
    if let Some(max_output_length) = input
        .get("max_output_length")
        .and_then(serde_json::Value::as_u64)
    {
        action.insert(
            "max_output_length".to_string(),
            serde_json::Value::Number(max_output_length.into()),
        );
    }
    Some((
        call_id.clone(),
        serde_json::json!({
            "type": "shell_call",
            "call_id": call_id,
            "action": serde_json::Value::Object(action),
            "status": "completed",
        }),
        RuntimeAnthropicNativeClientToolCall {
            kind: RuntimeAnthropicNativeClientToolKind::Shell,
            max_output_length: input
                .get("max_output_length")
                .and_then(serde_json::Value::as_u64),
        },
    ))
}

pub(super) fn runtime_proxy_anthropic_coordinate_component(
    value: &serde_json::Value,
) -> Option<i64> {
    value.as_i64().or_else(|| {
        value
            .as_u64()
            .and_then(|component| i64::try_from(component).ok())
    })
}

pub(super) fn runtime_proxy_anthropic_coordinate_pair(
    value: Option<&serde_json::Value>,
) -> Option<(i64, i64)> {
    let coordinates = value?.as_array()?;
    if coordinates.len() < 2 {
        return None;
    }
    let x = runtime_proxy_anthropic_coordinate_component(coordinates.first()?)?;
    let y = runtime_proxy_anthropic_coordinate_component(coordinates.get(1)?)?;
    Some((x, y))
}

pub(super) fn runtime_proxy_anthropic_computer_keypress_keys(
    key_combo: &str,
) -> Option<Vec<String>> {
    let keys = key_combo
        .split('+')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_uppercase())
        .collect::<Vec<_>>();
    (!keys.is_empty()).then_some(keys)
}

pub(super) fn runtime_proxy_translate_anthropic_computer_action(
    input: &serde_json::Map<String, serde_json::Value>,
) -> Option<serde_json::Value> {
    let action = input
        .get("action")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    match action {
        "screenshot" => Some(serde_json::json!({ "type": "screenshot" })),
        "left_click" | "right_click" | "middle_click" => {
            let (x, y) = runtime_proxy_anthropic_coordinate_pair(input.get("coordinate"))?;
            let button = match action {
                "left_click" => "left",
                "right_click" => "right",
                "middle_click" => "middle",
                _ => unreachable!(),
            };
            Some(serde_json::json!({
                "type": "click",
                "button": button,
                "x": x,
                "y": y,
            }))
        }
        "double_click" => {
            let (x, y) = runtime_proxy_anthropic_coordinate_pair(input.get("coordinate"))?;
            Some(serde_json::json!({
                "type": "double_click",
                "x": x,
                "y": y,
            }))
        }
        "mouse_move" => {
            let (x, y) = runtime_proxy_anthropic_coordinate_pair(input.get("coordinate"))?;
            Some(serde_json::json!({
                "type": "move",
                "x": x,
                "y": y,
            }))
        }
        "type" => input
            .get("text")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|text| {
                serde_json::json!({
                    "type": "type",
                    "text": text,
                })
            }),
        "key" => input
            .get("key")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .and_then(runtime_proxy_anthropic_computer_keypress_keys)
            .map(|keys| {
                serde_json::json!({
                    "type": "keypress",
                    "keys": keys,
                })
            }),
        "wait" => Some(serde_json::json!({ "type": "wait" })),
        _ => None,
    }
}

pub(super) fn runtime_proxy_translate_anthropic_computer_tool_call(
    block: &serde_json::Value,
) -> Option<(
    String,
    serde_json::Value,
    RuntimeAnthropicNativeClientToolCall,
)> {
    let name = block
        .get("name")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    if runtime_proxy_anthropic_client_tool_name(name) != Some("computer") {
        return None;
    }
    let call_id = block
        .get("id")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())?
        .to_string();
    let input = block.get("input")?.as_object()?;
    let action = runtime_proxy_translate_anthropic_computer_action(input)?;
    Some((
        call_id.clone(),
        serde_json::json!({
            "type": "computer_call",
            "call_id": call_id,
            "actions": [action],
            "status": "completed",
        }),
        RuntimeAnthropicNativeClientToolCall {
            kind: RuntimeAnthropicNativeClientToolKind::Computer,
            max_output_length: None,
        },
    ))
}

pub(super) fn runtime_proxy_translate_anthropic_tool_result_payload(
    block: &serde_json::Value,
) -> Result<(String, String, Vec<serde_json::Value>)> {
    let call_id = block
        .get("tool_use_id")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .context("Anthropic tool_result block requires a non-empty tool_use_id")?
        .to_string();
    let mut output_text = String::new();
    let mut image_parts = Vec::new();

    match block.get("content") {
        Some(serde_json::Value::String(text)) => {
            output_text = runtime_proxy_normalize_anthropic_tool_result_text(text)
                .unwrap_or_else(|| text.clone());
        }
        Some(serde_json::Value::Array(items)) => {
            let (translated_output, translated_images) =
                runtime_proxy_translate_anthropic_tool_result_content(items);
            output_text = translated_output;
            image_parts.extend(translated_images);
        }
        Some(serde_json::Value::Object(object)) => {
            let mut normalized = object.clone();
            if let Some(text) = runtime_proxy_translate_anthropic_text_from_block(
                &serde_json::Value::Object(object.clone()),
            ) && !normalized.contains_key("text")
            {
                normalized.insert("text".to_string(), serde_json::Value::String(text));
            }
            output_text = serde_json::Value::Object(normalized).to_string();
        }
        Some(other) => {
            output_text = serde_json::to_string(other)
                .context("failed to serialize Anthropic tool_result content")?;
        }
        None => {}
    }

    if block
        .get("is_error")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
    {
        output_text = runtime_proxy_translate_anthropic_error_tool_result_output(output_text);
    }

    Ok((call_id, output_text, image_parts))
}

pub(super) fn runtime_proxy_translate_anthropic_error_tool_result_output(
    output_text: String,
) -> String {
    if output_text.is_empty() {
        return "Error".to_string();
    }
    if let Ok(mut structured) = serde_json::from_str::<serde_json::Value>(&output_text)
        && let Some(object) = structured.as_object_mut()
    {
        object.insert("is_error".to_string(), serde_json::Value::Bool(true));
        return structured.to_string();
    }
    format!("Error: {output_text}")
}

pub(super) fn runtime_proxy_translate_anthropic_shell_tool_result(
    block: &serde_json::Value,
    max_output_length: Option<u64>,
) -> Result<Vec<serde_json::Value>> {
    let (call_id, output_text, image_parts) =
        runtime_proxy_translate_anthropic_tool_result_payload(block)?;
    let is_error = block
        .get("is_error")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let command_output = serde_json::json!({
        "stdout": if is_error { "" } else { output_text.as_str() },
        "stderr": if is_error { output_text.as_str() } else { "" },
        "outcome": {
            "type": "exit",
            "exit_code": if is_error { 1 } else { 0 },
        },
    });
    let mut shell_output = serde_json::Map::new();
    shell_output.insert(
        "type".to_string(),
        serde_json::Value::String("shell_call_output".to_string()),
    );
    shell_output.insert("call_id".to_string(), serde_json::Value::String(call_id));
    shell_output.insert(
        "output".to_string(),
        serde_json::Value::Array(vec![command_output]),
    );
    if let Some(max_output_length) = max_output_length {
        shell_output.insert(
            "max_output_length".to_string(),
            serde_json::Value::Number(max_output_length.into()),
        );
    }
    let mut translated = vec![serde_json::Value::Object(shell_output)];
    if !image_parts.is_empty() {
        translated.push(serde_json::json!({
            "role": "user",
            "content": image_parts,
        }));
    }
    Ok(translated)
}

pub(super) fn runtime_proxy_translate_anthropic_computer_tool_result(
    block: &serde_json::Value,
) -> Result<Option<Vec<serde_json::Value>>> {
    if block
        .get("is_error")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
    {
        return Ok(None);
    }
    let call_id = block
        .get("tool_use_id")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .context("Anthropic tool_result block requires a non-empty tool_use_id")?
        .to_string();
    let image_url = match block.get("content") {
        Some(serde_json::Value::Array(items)) if items.len() == 1 => items
            .first()
            .and_then(runtime_proxy_anthropic_image_data_url),
        Some(serde_json::Value::Object(object)) => {
            runtime_proxy_anthropic_image_data_url(&serde_json::Value::Object(object.clone()))
        }
        _ => None,
    };
    let Some(image_url) = image_url else {
        return Ok(None);
    };
    Ok(Some(vec![serde_json::json!({
        "type": "computer_call_output",
        "call_id": call_id,
        "output": {
            "type": "computer_screenshot",
            "image_url": image_url,
            "detail": "original",
        },
    })]))
}

pub(super) fn runtime_proxy_translate_anthropic_message_content(
    role: &str,
    content: &serde_json::Value,
    native_shell_enabled: bool,
    native_computer_enabled: bool,
    native_client_tool_calls: &mut BTreeMap<String, RuntimeAnthropicNativeClientToolCall>,
) -> Result<Vec<serde_json::Value>> {
    if let Some(text) = content.as_str() {
        return Ok(vec![serde_json::json!({
            "role": role,
            "content": text,
        })]);
    }

    let blocks = content
        .as_array()
        .context("Anthropic message content must be a string or an array of content blocks")?;
    let mut input_items = Vec::new();
    let has_tool_blocks = blocks.iter().any(|block| {
        block
            .get("type")
            .and_then(serde_json::Value::as_str)
            .is_some_and(runtime_proxy_anthropic_is_special_input_item_block_type)
    });

    match role {
        "user" => {
            if let Some(message_content) =
                runtime_proxy_translate_anthropic_user_content_blocks(blocks)
            {
                input_items.push(serde_json::json!({
                    "role": "user",
                    "content": message_content,
                }));
            } else if !has_tool_blocks {
                input_items.push(serde_json::json!({
                    "role": "user",
                    "content": "",
                }));
            }
        }
        "assistant" => {
            let text = runtime_proxy_translate_anthropic_text_blocks(blocks);
            if !text.is_empty() || !has_tool_blocks {
                input_items.push(serde_json::json!({
                    "role": "assistant",
                    "content": text,
                }));
            }
        }
        other => bail!("Unsupported Anthropic role '{other}'"),
    }

    for block in blocks {
        match block.get("type").and_then(serde_json::Value::as_str) {
            Some(block_type) if runtime_proxy_anthropic_is_tool_use_block_type(block_type) => {
                if native_shell_enabled
                    && let Some((call_id, translated, native_shell_call)) =
                        runtime_proxy_translate_anthropic_shell_tool_call(block)
                {
                    native_client_tool_calls.insert(call_id, native_shell_call);
                    input_items.push(translated);
                    continue;
                }
                if native_computer_enabled
                    && let Some((call_id, translated, native_computer_call)) =
                        runtime_proxy_translate_anthropic_computer_tool_call(block)
                {
                    native_client_tool_calls.insert(call_id, native_computer_call);
                    input_items.push(translated);
                    continue;
                }
                input_items.push(runtime_proxy_translate_anthropic_tool_call(block)?);
            }
            Some(block_type) if runtime_proxy_anthropic_is_tool_result_block_type(block_type) => {
                let native_client_tool_call = block
                    .get("tool_use_id")
                    .and_then(serde_json::Value::as_str)
                    .and_then(|call_id| native_client_tool_calls.get(call_id));
                match native_client_tool_call.map(|call| call.kind) {
                    Some(RuntimeAnthropicNativeClientToolKind::Shell) if native_shell_enabled => {
                        input_items.extend(runtime_proxy_translate_anthropic_shell_tool_result(
                            block,
                            native_client_tool_call.and_then(|call| call.max_output_length),
                        )?);
                    }
                    Some(RuntimeAnthropicNativeClientToolKind::Computer)
                        if native_computer_enabled =>
                    {
                        if let Some(translated) =
                            runtime_proxy_translate_anthropic_computer_tool_result(block)?
                        {
                            input_items.extend(translated);
                        } else {
                            input_items
                                .extend(runtime_proxy_translate_anthropic_tool_result(block)?);
                        }
                    }
                    _ => {
                        input_items.extend(runtime_proxy_translate_anthropic_tool_result(block)?);
                    }
                }
            }
            Some("mcp_approval_response") => {
                input_items.push(runtime_proxy_translate_anthropic_mcp_approval_response(
                    block,
                )?);
            }
            _ => {}
        }
    }

    Ok(input_items)
}

pub(super) fn runtime_proxy_translate_anthropic_user_content_blocks(
    blocks: &[serde_json::Value],
) -> Option<serde_json::Value> {
    let mut text_blocks = Vec::new();
    let mut parts = Vec::new();
    let mut saw_image = false;

    for block in blocks {
        if block.get("type").and_then(serde_json::Value::as_str) == Some("mcp_approval_response") {
            continue;
        }
        if block.get("type").and_then(serde_json::Value::as_str) == Some("image") {
            if !saw_image {
                for text in text_blocks.drain(..) {
                    parts.push(serde_json::json!({
                        "type": "input_text",
                        "text": text,
                    }));
                }
            }
            saw_image = true;
            if let Some(part) = runtime_proxy_translate_anthropic_image_part(block) {
                parts.push(part);
            }
            continue;
        }

        if let Some(text) = runtime_proxy_translate_anthropic_text_from_block(block) {
            if saw_image {
                parts.push(serde_json::json!({
                    "type": "input_text",
                    "text": text,
                }));
            } else {
                text_blocks.push(text);
            }
        } else if let Some(text) = runtime_proxy_translate_anthropic_block_fallback_text(block) {
            if saw_image {
                parts.push(serde_json::json!({
                    "type": "input_text",
                    "text": text,
                }));
            } else {
                text_blocks.push(text);
            }
        }
    }

    if saw_image {
        (!parts.is_empty()).then_some(serde_json::Value::Array(parts))
    } else {
        let text = text_blocks.join("\n");
        (!text.is_empty()).then_some(serde_json::Value::String(text))
    }
}

pub(super) fn runtime_proxy_translate_anthropic_text_blocks(
    blocks: &[serde_json::Value],
) -> String {
    blocks
        .iter()
        .filter_map(runtime_proxy_translate_anthropic_text_from_block)
        .collect::<Vec<_>>()
        .join("\n")
}

pub(super) fn runtime_proxy_translate_anthropic_tool_call(
    block: &serde_json::Value,
) -> Result<serde_json::Value> {
    let block_type = block
        .get("type")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("tool_use");
    let name = block
        .get("name")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| {
            if block_type == "server_tool_use" {
                runtime_proxy_anthropic_builtin_server_tool_name(value).unwrap_or(value)
            } else {
                value
            }
        })
        .with_context(|| format!("Anthropic {block_type} block requires a non-empty name"))?;
    let call_id = block
        .get("id")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .with_context(|| format!("Anthropic {block_type} block requires a non-empty id"))?;
    let mut input = block
        .get("input")
        .cloned()
        .unwrap_or_else(|| serde_json::Value::Object(serde_json::Map::new()));
    if block_type == "mcp_tool_use"
        && let Some(server_name) = block
            .get("server_name")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        && let Some(object) = input.as_object_mut()
    {
        object
            .entry("server_name".to_string())
            .or_insert_with(|| serde_json::Value::String(server_name.to_string()));
    }
    let arguments = serde_json::to_string(&input)
        .with_context(|| format!("failed to serialize Anthropic {block_type} input"))?;
    Ok(serde_json::json!({
        "type": "function_call",
        "call_id": call_id,
        "name": name,
        "arguments": arguments,
    }))
}

pub(super) fn runtime_proxy_translate_anthropic_tool_result_content(
    items: &[serde_json::Value],
) -> (String, Vec<serde_json::Value>) {
    let mut text_parts = Vec::new();
    let mut tool_references = Vec::new();
    let mut structured_blocks = Vec::new();
    let mut image_parts = Vec::new();
    let mut content_blocks = Vec::new();

    for item in items {
        match item.get("type").and_then(serde_json::Value::as_str) {
            Some("tool_reference") => {
                if let Some(tool_name) = item
                    .get("tool_name")
                    .and_then(serde_json::Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                {
                    tool_references.push(tool_name.to_string());
                } else {
                    structured_blocks.push(item.clone());
                }
                content_blocks.push(item.clone());
            }
            Some("image") => {
                if let Some(part) = runtime_proxy_translate_anthropic_image_part(item) {
                    image_parts.push(part);
                }
            }
            _ => {
                if let Some(text) = runtime_proxy_translate_anthropic_text_from_block(item) {
                    text_parts.push(text);
                    content_blocks.push(item.clone());
                    if item.get("type").and_then(serde_json::Value::as_str) == Some("document")
                        || item.get("type").and_then(serde_json::Value::as_str)
                            == Some("web_fetch_result")
                    {
                        structured_blocks.push(item.clone());
                    }
                } else {
                    structured_blocks.push(item.clone());
                    content_blocks.push(item.clone());
                }
            }
        }
    }

    let text = text_parts.join("\n");
    if tool_references.is_empty() && structured_blocks.is_empty() {
        return (text, image_parts);
    }

    let mut output = serde_json::Map::new();
    if !text.is_empty() {
        output.insert("text".to_string(), serde_json::Value::String(text));
    }
    let has_tool_references = !tool_references.is_empty();
    if has_tool_references {
        output.insert(
            "tool_references".to_string(),
            serde_json::Value::Array(
                tool_references
                    .into_iter()
                    .map(serde_json::Value::String)
                    .collect(),
            ),
        );
    }
    if !content_blocks.is_empty() && (has_tool_references || !structured_blocks.is_empty()) {
        output.insert(
            "content_blocks".to_string(),
            serde_json::Value::Array(content_blocks),
        );
    }

    (serde_json::Value::Object(output).to_string(), image_parts)
}

pub(super) fn runtime_proxy_extract_balanced_json_array_bounds(
    text: &str,
    start: usize,
) -> Option<(usize, usize)> {
    if text.as_bytes().get(start).copied() != Some(b'[') {
        return None;
    }

    let mut depth = 0usize;
    let mut in_string = false;
    let mut escape = false;

    for (offset, ch) in text[start..].char_indices() {
        if in_string {
            if escape {
                escape = false;
                continue;
            }
            match ch {
                '\\' => escape = true,
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }

        match ch {
            '"' => in_string = true,
            '[' => depth += 1,
            ']' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    let end = start + offset + ch.len_utf8();
                    return Some((start, end));
                }
            }
            _ => {}
        }
    }

    None
}

pub(super) fn runtime_proxy_anthropic_web_search_query_from_tool_result_text(
    text: &str,
) -> Option<String> {
    let prefix = "Web search results for query:";
    let remainder = text.trim().strip_prefix(prefix)?.trim_start();
    let first_line = remainder.lines().next()?.trim();
    if first_line.is_empty() {
        return None;
    }
    if let Some(stripped) = first_line.strip_prefix('"')
        && let Some(end_quote) = stripped.find('"')
    {
        let query = stripped[..end_quote].trim();
        if !query.is_empty() {
            return Some(query.to_string());
        }
    }

    let query = first_line.trim_matches('"').trim();
    (!query.is_empty()).then(|| query.to_string())
}

pub(super) fn runtime_proxy_anthropic_web_search_urls_from_tool_result_text(
    text: &str,
) -> (Vec<String>, usize) {
    let mut urls = Vec::new();
    let mut seen = BTreeSet::new();
    let mut search_from = 0usize;
    let mut last_array_end = 0usize;

    while let Some(links_offset) = text[search_from..].find("Links:") {
        let links_start = search_from + links_offset;
        let Some(array_offset) = text[links_start..].find('[') else {
            search_from = links_start.saturating_add("Links:".len());
            continue;
        };
        let array_start = links_start + array_offset;
        let Some((_, array_end)) =
            runtime_proxy_extract_balanced_json_array_bounds(text, array_start)
        else {
            break;
        };
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text[array_start..array_end])
            && let Some(items) = value.as_array()
        {
            for item in items {
                let Some(url) = item
                    .get("url")
                    .and_then(serde_json::Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                else {
                    continue;
                };
                if seen.insert(url.to_string()) {
                    urls.push(url.to_string());
                }
            }
        }
        last_array_end = array_end;
        search_from = array_end;
    }

    (urls, last_array_end)
}

pub(super) fn runtime_proxy_compact_web_search_tool_result_summary(summary: &str) -> String {
    let mut compact_lines = Vec::new();
    let mut saw_content = false;

    for raw_line in summary.lines() {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() {
            if saw_content
                && compact_lines
                    .last()
                    .is_some_and(|line: &String| !line.is_empty())
            {
                compact_lines.push(String::new());
            }
            continue;
        }
        if trimmed == "No links found." || trimmed.starts_with("Link:") {
            continue;
        }
        if trimmed == "Sources:"
            || trimmed.starts_with("REMINDER:")
            || trimmed.starts_with("Kalau mau, saya bisa lanjutkan")
            || trimmed.starts_with("If you'd like")
            || trimmed.starts_with("If you want,")
        {
            break;
        }
        compact_lines.push(trimmed.to_string());
        saw_content = true;
    }

    while compact_lines.last().is_some_and(|line| line.is_empty()) {
        compact_lines.pop();
    }

    compact_lines.join("\n")
}

pub(super) fn runtime_proxy_normalize_anthropic_tool_result_text(text: &str) -> Option<String> {
    let query = runtime_proxy_anthropic_web_search_query_from_tool_result_text(text)?;
    let (urls, last_array_end) =
        runtime_proxy_anthropic_web_search_urls_from_tool_result_text(text);
    let summary_source = if last_array_end > 0 {
        &text[last_array_end..]
    } else {
        text
    };
    let summary = runtime_proxy_compact_web_search_tool_result_summary(summary_source);
    if urls.is_empty() && summary.is_empty() {
        return None;
    }

    let mut output = serde_json::Map::new();
    output.insert("query".to_string(), serde_json::Value::String(query));
    if !summary.is_empty() {
        output.insert("text".to_string(), serde_json::Value::String(summary));
    }
    if !urls.is_empty() {
        output.insert(
            "content_blocks".to_string(),
            serde_json::Value::Array(
                urls.into_iter()
                    .map(|url| {
                        serde_json::json!({
                            "type": "web_search_result",
                            "url": url,
                        })
                    })
                    .collect(),
            ),
        );
    }

    Some(serde_json::Value::Object(output).to_string())
}

pub(super) fn runtime_proxy_translate_anthropic_tool_result(
    block: &serde_json::Value,
) -> Result<Vec<serde_json::Value>> {
    let (call_id, output_text, image_parts) =
        runtime_proxy_translate_anthropic_tool_result_payload(block)?;
    let mut translated = vec![serde_json::json!({
        "type": "function_call_output",
        "call_id": call_id,
        "output": output_text,
    })];
    if !image_parts.is_empty() {
        translated.push(serde_json::json!({
            "role": "user",
            "content": image_parts,
        }));
    }
    Ok(translated)
}

pub(super) fn runtime_proxy_anthropic_tool_use_server_tool_usage(
    block: &serde_json::Value,
) -> RuntimeAnthropicServerToolUsage {
    let tool_name = match block.get("type").and_then(serde_json::Value::as_str) {
        Some(block_type) if runtime_proxy_anthropic_is_tool_use_block_type(block_type) => block
            .get("name")
            .and_then(serde_json::Value::as_str)
            .map(str::trim),
        _ => None,
    };
    match tool_name.and_then(runtime_proxy_anthropic_builtin_server_tool_name) {
        Some("web_search") => RuntimeAnthropicServerToolUsage {
            web_search_requests: 1,
            ..RuntimeAnthropicServerToolUsage::default()
        },
        Some("web_fetch") => RuntimeAnthropicServerToolUsage {
            web_fetch_requests: 1,
            ..RuntimeAnthropicServerToolUsage::default()
        },
        Some("code_execution" | "bash_code_execution" | "text_editor_code_execution") => {
            RuntimeAnthropicServerToolUsage {
                code_execution_requests: 1,
                ..RuntimeAnthropicServerToolUsage::default()
            }
        }
        Some("tool_search_tool_regex" | "tool_search_tool_bm25") => {
            RuntimeAnthropicServerToolUsage {
                tool_search_requests: 1,
                ..RuntimeAnthropicServerToolUsage::default()
            }
        }
        _ => RuntimeAnthropicServerToolUsage::default(),
    }
}

pub(super) fn runtime_proxy_anthropic_register_server_tools_from_messages(
    messages: &[serde_json::Value],
    server_tools: &mut RuntimeAnthropicServerTools,
) {
    for message in messages {
        let Some(blocks) = message.get("content").and_then(serde_json::Value::as_array) else {
            continue;
        };
        for block in blocks {
            let block_type = block
                .get("type")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default();
            if !matches!(block_type, "server_tool_use" | "mcp_tool_use") {
                continue;
            }
            let Some(tool_name) = block
                .get("name")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
            else {
                continue;
            };
            let response_name =
                runtime_proxy_anthropic_builtin_server_tool_name(tool_name).unwrap_or(tool_name);
            server_tools.register_with_block_type(tool_name, response_name, block_type);
        }
    }
}

pub(super) fn runtime_proxy_anthropic_message_has_tool_chain_blocks(
    message: &serde_json::Value,
) -> bool {
    let Some(content) = message.get("content") else {
        return false;
    };
    let Some(blocks) = content.as_array() else {
        return false;
    };
    blocks.iter().any(|block| {
        block
            .get("type")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|block_type| {
                runtime_proxy_anthropic_is_tool_use_block_type(block_type)
                    || runtime_proxy_anthropic_is_tool_result_block_type(block_type)
            })
    })
}

pub(super) fn runtime_proxy_anthropic_carried_server_tool_usage(
    messages: &[serde_json::Value],
) -> RuntimeAnthropicServerToolUsage {
    let mut usage = RuntimeAnthropicServerToolUsage::default();
    let mut collecting_suffix = false;

    for message in messages.iter().rev() {
        let Some(blocks) = message.get("content").and_then(serde_json::Value::as_array) else {
            if collecting_suffix {
                break;
            }
            continue;
        };
        let mut saw_tool_chain_block = false;
        for block in blocks {
            let block_usage = runtime_proxy_anthropic_tool_use_server_tool_usage(block);
            if block_usage != RuntimeAnthropicServerToolUsage::default() {
                usage.add_assign(block_usage);
                saw_tool_chain_block = true;
                continue;
            }
            if block
                .get("type")
                .and_then(serde_json::Value::as_str)
                .is_some_and(runtime_proxy_anthropic_is_tool_result_block_type)
            {
                saw_tool_chain_block = true;
            }
        }
        if saw_tool_chain_block {
            collecting_suffix = true;
        } else if collecting_suffix
            || runtime_proxy_anthropic_message_has_tool_chain_blocks(message)
        {
            break;
        }
    }

    usage
}

pub(super) fn translate_runtime_anthropic_messages_request(
    request: &RuntimeProxyRequest,
) -> Result<RuntimeAnthropicMessagesRequest> {
    let value = serde_json::from_slice::<serde_json::Value>(&request.body)
        .context("failed to parse Anthropic request body as JSON")?;
    let requested_model = value
        .get("model")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .context("Anthropic request requires a non-empty model")?
        .to_string();
    let messages = value
        .get("messages")
        .and_then(serde_json::Value::as_array)
        .context("Anthropic request requires a messages array")?;
    if messages.is_empty() {
        bail!("Anthropic request requires at least one message");
    }
    let carried_server_tool_usage = runtime_proxy_anthropic_carried_server_tool_usage(messages);
    let target_model = runtime_proxy_claude_target_model(&requested_model);
    let native_shell_enabled = runtime_proxy_anthropic_native_shell_enabled_for_request(&value);
    let native_computer_enabled =
        runtime_proxy_anthropic_native_computer_enabled_for_request(&value, &target_model);

    let mut input = Vec::new();
    let mut native_client_tool_calls = BTreeMap::new();
    for message in messages {
        let role = message
            .get("role")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .context("Anthropic message is missing a non-empty role")?;
        let content = message
            .get("content")
            .context("Anthropic message is missing content")?;
        input.extend(runtime_proxy_translate_anthropic_message_content(
            role,
            content,
            native_shell_enabled,
            native_computer_enabled,
            &mut native_client_tool_calls,
        )?);
    }
    if input.is_empty() {
        input.push(serde_json::json!({
            "role": "user",
            "content": "",
        }));
    }

    let mut translated_body = serde_json::Map::new();
    translated_body.insert(
        "model".to_string(),
        serde_json::Value::String(target_model.clone()),
    );
    translated_body.insert("input".to_string(), serde_json::Value::Array(input));
    translated_body.insert(
        "stream".to_string(),
        serde_json::Value::Bool(
            value
                .get("stream")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false),
        ),
    );
    translated_body.insert("store".to_string(), serde_json::Value::Bool(false));
    let base_instructions = runtime_proxy_anthropic_system_instructions(&value)?;
    let translated_tools = runtime_proxy_translate_anthropic_tools(
        &value,
        native_shell_enabled,
        native_computer_enabled,
    )?;
    let mut translated_tools = translated_tools;
    runtime_proxy_anthropic_register_server_tools_from_messages(
        messages,
        &mut translated_tools.server_tools,
    );
    if let Some(instructions) =
        runtime_proxy_anthropic_append_tool_instructions(base_instructions, translated_tools.memory)
    {
        translated_body.insert(
            "instructions".to_string(),
            serde_json::Value::String(instructions),
        );
    }
    if !translated_tools.tools.is_empty() {
        translated_body.insert(
            "tools".to_string(),
            serde_json::Value::Array(translated_tools.tools.clone()),
        );
    }
    if translated_tools.server_tools.web_search {
        translated_body.insert(
            "include".to_string(),
            serde_json::json!(["web_search_call.action.sources"]),
        );
    }
    if let Some(tool_choice) = runtime_proxy_translate_anthropic_tool_choice(
        &value,
        &translated_tools.server_tools,
        &translated_tools.tool_name_aliases,
        &translated_tools.native_tool_names,
    )?
    .or_else(|| translated_tools.implicit_tool_choice())
    {
        translated_body.insert("tool_choice".to_string(), tool_choice);
    }
    if let Some(effort) = runtime_proxy_anthropic_reasoning_effort(&value, &target_model) {
        translated_body.insert(
            "reasoning".to_string(),
            serde_json::json!({
                "summary": "auto",
                "effort": effort,
            }),
        );
    } else if runtime_proxy_anthropic_wants_thinking(&value) {
        translated_body.insert(
            "reasoning".to_string(),
            serde_json::json!({
                "summary": "auto",
            }),
        );
    }

    let mut translated_headers = vec![("Content-Type".to_string(), "application/json".to_string())];
    if let Some(user_agent) = runtime_proxy_request_header_value(&request.headers, "User-Agent") {
        translated_headers.push(("User-Agent".to_string(), user_agent.to_string()));
    }
    if let Some(session_id) = runtime_proxy_claude_session_id(request) {
        translated_headers.push(("session_id".to_string(), session_id));
    }
    translated_headers.push((
        PRODEX_INTERNAL_REQUEST_ORIGIN_HEADER.to_string(),
        PRODEX_INTERNAL_REQUEST_ORIGIN_ANTHROPIC_MESSAGES.to_string(),
    ));

    Ok(RuntimeAnthropicMessagesRequest {
        translated_request: RuntimeProxyRequest {
            method: request.method.clone(),
            path_and_query: format!("{RUNTIME_PROXY_OPENAI_UPSTREAM_PATH}/responses"),
            headers: translated_headers,
            body: serde_json::to_vec(&serde_json::Value::Object(translated_body))
                .context("failed to serialize translated Anthropic request")?,
        },
        requested_model,
        stream: value
            .get("stream")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
        want_thinking: runtime_proxy_anthropic_wants_thinking(&value),
        server_tools: translated_tools.server_tools,
        carried_web_search_requests: carried_server_tool_usage.web_search_requests,
        carried_web_fetch_requests: carried_server_tool_usage.web_fetch_requests,
        carried_code_execution_requests: carried_server_tool_usage.code_execution_requests,
        carried_tool_search_requests: carried_server_tool_usage.tool_search_requests,
    })
}

pub(super) fn runtime_anthropic_message_id() -> String {
    format!("msg_{}", runtime_random_token("claude").replace('-', ""))
}

pub(super) fn runtime_anthropic_error_type_for_status(status: u16) -> &'static str {
    match status {
        400 => "invalid_request_error",
        401 => "authentication_error",
        403 => "permission_error",
        404 => "not_found_error",
        429 => "rate_limit_error",
        500 | 502 | 503 | 504 | 529 => "overloaded_error",
        _ => "api_error",
    }
}

pub(super) fn runtime_anthropic_error_message_from_parts(
    parts: &RuntimeBufferedResponseParts,
) -> String {
    if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&parts.body) {
        if let Some(message) = value
            .get("error")
            .and_then(|error| error.get("message"))
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return message.to_string();
        }
        if let Some(message) = value
            .get("message")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return message.to_string();
        }
    }
    let body = String::from_utf8_lossy(&parts.body).trim().to_string();
    if body.is_empty() {
        "Upstream runtime proxy request failed.".to_string()
    } else {
        body
    }
}

pub(super) fn build_runtime_anthropic_error_parts(
    status: u16,
    error_type: &str,
    message: &str,
) -> RuntimeBufferedResponseParts {
    RuntimeBufferedResponseParts {
        status,
        headers: vec![("Content-Type".to_string(), b"application/json".to_vec())],
        body: serde_json::json!({
            "type": "error",
            "error": {
                "type": error_type,
                "message": message,
            }
        })
        .to_string()
        .into_bytes(),
    }
}

pub(super) fn runtime_anthropic_error_from_upstream_parts(
    parts: RuntimeBufferedResponseParts,
) -> RuntimeBufferedResponseParts {
    let status = parts.status;
    let message = runtime_anthropic_error_message_from_parts(&parts);
    build_runtime_anthropic_error_parts(
        status,
        runtime_anthropic_error_type_for_status(status),
        &message,
    )
}

pub(super) fn runtime_anthropic_usage_from_value(
    value: &serde_json::Value,
) -> (u64, u64, Option<u64>) {
    let usage = value.get("usage").or_else(|| {
        value
            .get("response")
            .and_then(|response| response.get("usage"))
    });
    let input_tokens = usage
        .and_then(|usage| usage.get("input_tokens"))
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let output_tokens = usage
        .and_then(|usage| usage.get("output_tokens"))
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let cached_tokens = usage
        .and_then(|usage| usage.get("input_tokens_details"))
        .and_then(|details| details.get("cached_tokens"))
        .and_then(serde_json::Value::as_u64);
    (input_tokens, output_tokens, cached_tokens)
}

pub(super) fn runtime_anthropic_tool_usage_web_search_requests_from_value(
    value: &serde_json::Value,
) -> u64 {
    value
        .get("tool_usage")
        .or_else(|| {
            value
                .get("response")
                .and_then(|response| response.get("tool_usage"))
        })
        .and_then(|tool_usage| tool_usage.get("web_search"))
        .and_then(|web_search| web_search.get("num_requests"))
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0)
}

pub(super) fn runtime_anthropic_tool_usage_tool_search_requests_from_value(
    value: &serde_json::Value,
) -> u64 {
    value
        .get("tool_usage")
        .or_else(|| {
            value
                .get("response")
                .and_then(|response| response.get("tool_usage"))
        })
        .and_then(|tool_usage| tool_usage.get("tool_search"))
        .and_then(|tool_search| tool_search.get("num_requests"))
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0)
}

pub(super) fn runtime_anthropic_tool_usage_code_execution_requests_from_value(
    value: &serde_json::Value,
) -> u64 {
    value
        .get("tool_usage")
        .or_else(|| {
            value
                .get("response")
                .and_then(|response| response.get("tool_usage"))
        })
        .and_then(|tool_usage| tool_usage.get("code_execution"))
        .and_then(|code_execution| code_execution.get("num_requests"))
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0)
}

pub(super) fn runtime_anthropic_server_tool_registration_for_call(
    tool_name: &str,
    server_tools: Option<&RuntimeAnthropicServerTools>,
) -> Option<(String, String)> {
    if let Some(server_tools) = server_tools
        && let Some(registration) = server_tools.registration_for_call(tool_name)
    {
        return Some((
            registration.response_name.clone(),
            registration.block_type.clone(),
        ));
    }
    runtime_proxy_anthropic_builtin_server_tool_name(tool_name)
        .map(|name| (name.to_string(), "server_tool_use".to_string()))
}

pub(super) fn runtime_anthropic_server_tool_name_for_call(
    tool_name: &str,
    server_tools: Option<&RuntimeAnthropicServerTools>,
) -> Option<String> {
    runtime_anthropic_server_tool_registration_for_call(tool_name, server_tools)
        .map(|(response_name, _)| response_name)
}

pub(super) fn runtime_anthropic_output_item_server_tool_usage(
    item: &serde_json::Value,
    server_tools: Option<&RuntimeAnthropicServerTools>,
) -> RuntimeAnthropicServerToolUsage {
    match item.get("type").and_then(serde_json::Value::as_str) {
        Some("web_search_call") => RuntimeAnthropicServerToolUsage {
            web_search_requests: runtime_anthropic_web_search_request_count_from_output_item(item),
            ..RuntimeAnthropicServerToolUsage::default()
        },
        Some("function_call") => match item
            .get("name")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .and_then(|name| runtime_anthropic_server_tool_name_for_call(name, server_tools))
            .as_deref()
        {
            Some("web_search") => RuntimeAnthropicServerToolUsage {
                web_search_requests: 1,
                ..RuntimeAnthropicServerToolUsage::default()
            },
            Some("web_fetch") => RuntimeAnthropicServerToolUsage {
                web_fetch_requests: 1,
                ..RuntimeAnthropicServerToolUsage::default()
            },
            Some("code_execution" | "bash_code_execution" | "text_editor_code_execution") => {
                RuntimeAnthropicServerToolUsage {
                    code_execution_requests: 1,
                    ..RuntimeAnthropicServerToolUsage::default()
                }
            }
            Some("tool_search_tool_regex" | "tool_search_tool_bm25") => {
                RuntimeAnthropicServerToolUsage {
                    tool_search_requests: 1,
                    ..RuntimeAnthropicServerToolUsage::default()
                }
            }
            _ => RuntimeAnthropicServerToolUsage::default(),
        },
        _ => RuntimeAnthropicServerToolUsage::default(),
    }
}

pub(super) fn runtime_anthropic_web_search_request_count_from_output_item(
    item: &serde_json::Value,
) -> u64 {
    let Some(action) = item.get("action") else {
        return 1;
    };
    action
        .get("queries")
        .and_then(serde_json::Value::as_array)
        .map(|queries| queries.len() as u64)
        .filter(|count| *count > 0)
        .unwrap_or(1)
}

pub(super) fn runtime_anthropic_web_search_request_count_from_output(
    output: &[serde_json::Value],
    server_tools: Option<&RuntimeAnthropicServerTools>,
) -> u64 {
    output
        .iter()
        .map(|item| {
            runtime_anthropic_output_item_server_tool_usage(item, server_tools).web_search_requests
        })
        .sum()
}

pub(super) fn runtime_anthropic_web_fetch_request_count_from_output(
    output: &[serde_json::Value],
    server_tools: Option<&RuntimeAnthropicServerTools>,
) -> u64 {
    output
        .iter()
        .map(|item| {
            runtime_anthropic_output_item_server_tool_usage(item, server_tools).web_fetch_requests
        })
        .sum()
}

pub(super) fn runtime_anthropic_tool_search_request_count_from_output(
    output: &[serde_json::Value],
    server_tools: Option<&RuntimeAnthropicServerTools>,
) -> u64 {
    output
        .iter()
        .map(|item| {
            runtime_anthropic_output_item_server_tool_usage(item, server_tools).tool_search_requests
        })
        .sum()
}

pub(super) fn runtime_anthropic_code_execution_request_count_from_output(
    output: &[serde_json::Value],
    server_tools: Option<&RuntimeAnthropicServerTools>,
) -> u64 {
    output
        .iter()
        .map(|item| {
            runtime_anthropic_output_item_server_tool_usage(item, server_tools)
                .code_execution_requests
        })
        .sum()
}

pub(super) fn runtime_anthropic_usage_json(
    input_tokens: u64,
    output_tokens: u64,
    cached_tokens: Option<u64>,
    web_search_requests: u64,
    web_fetch_requests: u64,
    code_execution_requests: u64,
    tool_search_requests: u64,
) -> serde_json::Map<String, serde_json::Value> {
    let mut usage = serde_json::Map::new();
    usage.insert(
        "input_tokens".to_string(),
        serde_json::Value::Number(input_tokens.into()),
    );
    usage.insert(
        "output_tokens".to_string(),
        serde_json::Value::Number(output_tokens.into()),
    );
    if let Some(cached_tokens) = cached_tokens {
        usage.insert(
            "cache_read_input_tokens".to_string(),
            serde_json::Value::Number(cached_tokens.into()),
        );
    }
    usage.insert("server_tool_use".to_string(), {
        let mut server_tool_use = serde_json::Map::new();
        server_tool_use.insert(
            "web_search_requests".to_string(),
            serde_json::Value::Number(web_search_requests.into()),
        );
        server_tool_use.insert(
            "web_fetch_requests".to_string(),
            serde_json::Value::Number(web_fetch_requests.into()),
        );
        if code_execution_requests > 0 {
            server_tool_use.insert(
                "code_execution_requests".to_string(),
                serde_json::Value::Number(code_execution_requests.into()),
            );
        }
        if tool_search_requests > 0 {
            server_tool_use.insert(
                "tool_search_requests".to_string(),
                serde_json::Value::Number(tool_search_requests.into()),
            );
        }
        serde_json::Value::Object(server_tool_use)
    });
    usage
}

pub(super) fn runtime_anthropic_tool_input_from_arguments(arguments: &str) -> serde_json::Value {
    serde_json::from_str::<serde_json::Value>(arguments)
        .ok()
        .filter(|value| value.is_object())
        .unwrap_or_else(|| serde_json::Value::Object(serde_json::Map::new()))
}

pub(super) fn runtime_anthropic_reasoning_summary_text(item: &serde_json::Value) -> String {
    item.get("summary")
        .and_then(serde_json::Value::as_array)
        .map(|summary| {
            summary
                .iter()
                .filter_map(|entry| {
                    entry
                        .get("text")
                        .and_then(serde_json::Value::as_str)
                        .or_else(|| {
                            (entry.get("type").and_then(serde_json::Value::as_str)
                                == Some("summary_text"))
                            .then(|| entry.get("text").and_then(serde_json::Value::as_str))
                            .flatten()
                        })
                })
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default()
}

pub(super) fn runtime_anthropic_message_annotation_titles_by_url(
    output: &[serde_json::Value],
) -> BTreeMap<String, String> {
    let mut titles = BTreeMap::new();
    for item in output {
        let Some(parts) = item.get("content").and_then(serde_json::Value::as_array) else {
            continue;
        };
        for part in parts {
            let Some(annotations) = part
                .get("annotations")
                .and_then(serde_json::Value::as_array)
            else {
                continue;
            };
            for annotation in annotations {
                let url = annotation
                    .get("url")
                    .and_then(serde_json::Value::as_str)
                    .or_else(|| {
                        annotation
                            .get("url_citation")
                            .and_then(|value| value.get("url"))
                            .and_then(serde_json::Value::as_str)
                    })
                    .map(str::trim)
                    .filter(|value| !value.is_empty());
                let title = annotation
                    .get("title")
                    .and_then(serde_json::Value::as_str)
                    .or_else(|| {
                        annotation
                            .get("url_citation")
                            .and_then(|value| value.get("title"))
                            .and_then(serde_json::Value::as_str)
                    })
                    .map(str::trim)
                    .filter(|value| !value.is_empty());
                if let (Some(url), Some(title)) = (url, title) {
                    titles
                        .entry(url.to_string())
                        .or_insert_with(|| title.to_string());
                }
            }
        }
    }
    titles
}

pub(super) fn runtime_anthropic_web_search_blocks_from_output_item(
    item: &serde_json::Value,
    annotation_titles_by_url: &BTreeMap<String, String>,
) -> Vec<serde_json::Value> {
    let call_id = item
        .get("id")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("web_search_call")
        .to_string();
    let queries = item
        .get("action")
        .and_then(|action| action.get("queries"))
        .and_then(serde_json::Value::as_array)
        .map(|queries| {
            queries
                .iter()
                .filter_map(|query| {
                    query
                        .as_str()
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(|value| serde_json::Value::String(value.to_string()))
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let query = item
        .get("action")
        .and_then(|action| action.get("query"))
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            queries
                .first()
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_default();

    let mut seen_urls = BTreeSet::new();
    let mut results = Vec::new();
    if let Some(sources) = item
        .get("action")
        .and_then(|action| action.get("sources"))
        .and_then(serde_json::Value::as_array)
    {
        for source in sources {
            let Some(url) = source
                .get("url")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
            else {
                continue;
            };
            if !seen_urls.insert(url.to_string()) {
                continue;
            }
            let mut result = serde_json::Map::new();
            result.insert(
                "type".to_string(),
                serde_json::Value::String("web_search_result".to_string()),
            );
            result.insert(
                "url".to_string(),
                serde_json::Value::String(url.to_string()),
            );
            if let Some(title) = source
                .get("title")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .or_else(|| annotation_titles_by_url.get(url).map(String::as_str))
            {
                result.insert(
                    "title".to_string(),
                    serde_json::Value::String(title.to_string()),
                );
            }
            for key in [
                "encrypted_content",
                "page_age",
                "snippet",
                "summary",
                "text",
            ] {
                if let Some(value) = source
                    .get(key)
                    .and_then(serde_json::Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                {
                    result.insert(
                        key.to_string(),
                        serde_json::Value::String(value.to_string()),
                    );
                }
            }
            results.push(serde_json::Value::Object(result));
        }
    }
    if results.is_empty() {
        for (url, title) in annotation_titles_by_url {
            if !seen_urls.insert(url.clone()) {
                continue;
            }
            results.push(serde_json::json!({
                "type": "web_search_result",
                "url": url,
                "title": title,
            }));
        }
    }

    vec![
        serde_json::json!({
            "type": "server_tool_use",
            "id": call_id,
            "name": "web_search",
            "input": {
                "query": query,
                "queries": queries,
            },
        }),
        serde_json::json!({
            "type": "web_search_tool_result",
            "tool_use_id": call_id,
            "content": results,
        }),
    ]
}

pub(super) fn runtime_anthropic_shell_tool_input_from_output_item(
    item: &serde_json::Value,
) -> serde_json::Value {
    let mut input = serde_json::Map::new();
    if let Some(action) = item.get("action").and_then(serde_json::Value::as_object) {
        let commands = action
            .get("commands")
            .and_then(serde_json::Value::as_array)
            .map(|commands| {
                commands
                    .iter()
                    .filter_map(|command| {
                        command
                            .as_str()
                            .map(str::trim)
                            .filter(|value| !value.is_empty())
                            .map(|value| serde_json::Value::String(value.to_string()))
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if !commands.is_empty() {
            let command_text = commands
                .iter()
                .filter_map(serde_json::Value::as_str)
                .collect::<Vec<_>>()
                .join("\n");
            input.insert(
                "command".to_string(),
                serde_json::Value::String(command_text),
            );
        } else if let Some(command) = action
            .get("command")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            input.insert(
                "command".to_string(),
                serde_json::Value::String(command.to_string()),
            );
        }
        if let Some(timeout_ms) = action.get("timeout_ms").and_then(serde_json::Value::as_u64) {
            input.insert(
                "timeout_ms".to_string(),
                serde_json::Value::Number(timeout_ms.into()),
            );
        }
        if let Some(max_output_length) = action
            .get("max_output_length")
            .and_then(serde_json::Value::as_u64)
        {
            input.insert(
                "max_output_length".to_string(),
                serde_json::Value::Number(max_output_length.into()),
            );
        }
    }
    serde_json::Value::Object(input)
}

pub(super) fn runtime_anthropic_shell_tool_use_block_from_output_item(
    item: &serde_json::Value,
) -> serde_json::Value {
    let call_id = item
        .get("call_id")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("shell_call");
    serde_json::json!({
        "type": "tool_use",
        "id": call_id,
        "name": "bash",
        "input": runtime_anthropic_shell_tool_input_from_output_item(item),
    })
}

pub(super) fn runtime_anthropic_computer_key_combo_from_output_action(
    action: &serde_json::Map<String, serde_json::Value>,
) -> Option<String> {
    let keys = action
        .get("keys")
        .and_then(serde_json::Value::as_array)?
        .iter()
        .filter_map(|key| {
            key.as_str()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(|value| value.to_ascii_lowercase())
        })
        .collect::<Vec<_>>();
    (!keys.is_empty()).then_some(keys.join("+"))
}

pub(super) fn runtime_anthropic_computer_tool_input_from_output_item(
    item: &serde_json::Value,
) -> Option<serde_json::Value> {
    let actions = item.get("actions").and_then(serde_json::Value::as_array)?;
    if actions.len() != 1 {
        return None;
    }
    let action = actions.first()?.as_object()?;
    let action_type = action
        .get("type")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    let input = match action_type {
        "screenshot" => serde_json::json!({ "action": "screenshot" }),
        "click" => {
            let button = action
                .get("button")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("left");
            let button_action = match button {
                "left" => "left_click",
                "right" => "right_click",
                "middle" => "middle_click",
                _ => return None,
            };
            let x = runtime_proxy_anthropic_coordinate_component(action.get("x")?)?;
            let y = runtime_proxy_anthropic_coordinate_component(action.get("y")?)?;
            serde_json::json!({
                "action": button_action,
                "coordinate": [x, y],
            })
        }
        "double_click" => {
            let x = runtime_proxy_anthropic_coordinate_component(action.get("x")?)?;
            let y = runtime_proxy_anthropic_coordinate_component(action.get("y")?)?;
            serde_json::json!({
                "action": "double_click",
                "coordinate": [x, y],
            })
        }
        "move" => {
            let x = runtime_proxy_anthropic_coordinate_component(action.get("x")?)?;
            let y = runtime_proxy_anthropic_coordinate_component(action.get("y")?)?;
            serde_json::json!({
                "action": "mouse_move",
                "coordinate": [x, y],
            })
        }
        "type" => serde_json::json!({
            "action": "type",
            "text": action
                .get("text")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())?,
        }),
        "keypress" => serde_json::json!({
            "action": "key",
            "key": runtime_anthropic_computer_key_combo_from_output_action(action)?,
        }),
        "wait" => serde_json::json!({
            "action": "wait",
        }),
        _ => return None,
    };
    Some(input)
}

pub(super) fn runtime_anthropic_raw_computer_tool_input_from_output_item(
    item: &serde_json::Value,
) -> serde_json::Value {
    item.get("actions")
        .filter(|value| value.is_array())
        .cloned()
        .map(|actions| serde_json::json!({ "actions": actions }))
        .unwrap_or_else(|| serde_json::json!({}))
}

pub(super) fn runtime_anthropic_computer_tool_use_block_from_output_item(
    item: &serde_json::Value,
) -> serde_json::Value {
    let call_id = item
        .get("call_id")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("computer_call");
    serde_json::json!({
        "type": "tool_use",
        "id": call_id,
        "name": "computer",
        "input": runtime_anthropic_computer_tool_input_from_output_item(item)
            .unwrap_or_else(|| runtime_anthropic_raw_computer_tool_input_from_output_item(item)),
    })
}

pub(super) fn runtime_anthropic_server_tool_use_block(
    call_id: &str,
    tool_name: &str,
    input: serde_json::Value,
    server_tools: Option<&RuntimeAnthropicServerTools>,
) -> Option<serde_json::Value> {
    runtime_anthropic_server_tool_registration_for_call(tool_name, server_tools).map(
        |(response_name, block_type)| {
            if block_type == "mcp_tool_use" {
                let mut input = input;
                let server_name = input
                    .get("server_name")
                    .and_then(serde_json::Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string);
                if let Some(object) = input.as_object_mut() {
                    object.remove("server_name");
                }
                let mut block = serde_json::Map::new();
                block.insert(
                    "type".to_string(),
                    serde_json::Value::String("mcp_tool_use".to_string()),
                );
                block.insert(
                    "id".to_string(),
                    serde_json::Value::String(call_id.to_string()),
                );
                block.insert("name".to_string(), serde_json::Value::String(response_name));
                block.insert("input".to_string(), input);
                if let Some(server_name) = server_name {
                    block.insert(
                        "server_name".to_string(),
                        serde_json::Value::String(server_name),
                    );
                }
                serde_json::Value::Object(block)
            } else {
                serde_json::json!({
                    "type": "server_tool_use",
                    "id": call_id,
                    "name": response_name,
                    "input": input,
                })
            }
        },
    )
}

pub(super) fn runtime_anthropic_mcp_call_blocks_from_output_item(
    item: &serde_json::Value,
) -> Vec<serde_json::Value> {
    let call_id = item
        .get("id")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("mcp_call")
        .to_string();
    let name = item
        .get("name")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("mcp_tool")
        .to_string();
    let server_name = item
        .get("server_label")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("mcp")
        .to_string();
    let input = runtime_anthropic_tool_input_from_arguments(
        item.get("arguments")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("{}"),
    );

    let mut content = vec![serde_json::json!({
        "type": "mcp_tool_use",
        "id": call_id,
        "name": name,
        "server_name": server_name,
        "input": input,
    })];

    let mut result_content = Vec::new();
    if let Some(output) = item
        .get("output")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        result_content.push(serde_json::json!({
            "type": "text",
            "text": output,
        }));
    }
    let is_error = item
        .get("error")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|error| {
            result_content.push(serde_json::json!({
                "type": "text",
                "text": error,
            }));
            true
        })
        .unwrap_or(false);
    if !result_content.is_empty() {
        content.push(serde_json::json!({
            "type": "mcp_tool_result",
            "tool_use_id": content
                .first()
                .and_then(|block| block.get("id"))
                .cloned()
                .unwrap_or_else(|| serde_json::Value::String("mcp_call".to_string())),
            "is_error": is_error,
            "content": result_content,
        }));
    }

    content
}

pub(super) fn runtime_anthropic_mcp_approval_request_block_from_output_item(
    item: &serde_json::Value,
) -> serde_json::Value {
    let id = item
        .get("id")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("mcp_approval_request");
    let name = item
        .get("name")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("mcp_tool");
    let server_name = item
        .get("server_label")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("mcp");
    let arguments = item
        .get("arguments")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .unwrap_or("{}");
    let input = runtime_anthropic_tool_input_from_arguments(arguments);
    serde_json::json!({
        "type": "mcp_approval_request",
        "id": id,
        "name": name,
        "server_name": server_name,
        "server_label": server_name,
        "arguments": arguments,
        "input": input,
    })
}

pub(super) fn runtime_anthropic_mcp_list_tools_block_from_output_item(
    item: &serde_json::Value,
) -> serde_json::Value {
    let id = item
        .get("id")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("mcp_list_tools");
    let server_name = item
        .get("server_label")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("mcp");
    let mut block = serde_json::Map::new();
    block.insert(
        "type".to_string(),
        serde_json::Value::String("mcp_list_tools".to_string()),
    );
    block.insert("id".to_string(), serde_json::Value::String(id.to_string()));
    block.insert(
        "server_name".to_string(),
        serde_json::Value::String(server_name.to_string()),
    );
    block.insert(
        "server_label".to_string(),
        serde_json::Value::String(server_name.to_string()),
    );
    if let Some(tools) = item.get("tools").filter(|value| value.is_array()).cloned() {
        block.insert("tools".to_string(), tools);
    }
    if let Some(error) = item
        .get("error")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        block.insert(
            "error".to_string(),
            serde_json::Value::String(error.to_string()),
        );
    }
    serde_json::Value::Object(block)
}

pub(super) fn runtime_anthropic_output_blocks_from_json(
    output: &[serde_json::Value],
    want_thinking: bool,
    server_tools: Option<&RuntimeAnthropicServerTools>,
) -> (Vec<serde_json::Value>, bool) {
    let mut content = Vec::new();
    let mut has_tool_calls = false;
    let annotation_titles_by_url = runtime_anthropic_message_annotation_titles_by_url(output);

    for item in output {
        match item.get("type").and_then(serde_json::Value::as_str) {
            Some("reasoning") if want_thinking => {
                let thinking = runtime_anthropic_reasoning_summary_text(item);
                if !thinking.is_empty() {
                    content.push(serde_json::json!({
                        "type": "thinking",
                        "thinking": thinking,
                    }));
                }
            }
            Some("message") => {
                if let Some(parts) = item.get("content").and_then(serde_json::Value::as_array) {
                    let mut text = String::new();
                    for part in parts {
                        if part
                            .get("type")
                            .and_then(serde_json::Value::as_str)
                            .is_some_and(|part_type| matches!(part_type, "output_text" | "text"))
                            && let Some(part_text) =
                                part.get("text").and_then(serde_json::Value::as_str)
                        {
                            text.push_str(part_text);
                        }
                    }
                    if !text.is_empty() {
                        content.push(serde_json::json!({
                            "type": "text",
                            "text": text,
                        }));
                    }
                }
            }
            Some("web_search_call") => {
                content.extend(runtime_anthropic_web_search_blocks_from_output_item(
                    item,
                    &annotation_titles_by_url,
                ));
            }
            Some("mcp_call") => {
                content.extend(runtime_anthropic_mcp_call_blocks_from_output_item(item));
            }
            Some("mcp_approval_request") => {
                has_tool_calls = true;
                content.push(runtime_anthropic_mcp_approval_request_block_from_output_item(item));
            }
            Some("mcp_list_tools") => {
                content.push(runtime_anthropic_mcp_list_tools_block_from_output_item(
                    item,
                ));
            }
            Some("shell_call") => {
                has_tool_calls = true;
                content.push(runtime_anthropic_shell_tool_use_block_from_output_item(
                    item,
                ));
            }
            Some("computer_call") => {
                has_tool_calls = true;
                content.push(runtime_anthropic_computer_tool_use_block_from_output_item(
                    item,
                ));
            }
            Some("function_call") => {
                has_tool_calls = true;
                let call_id = item
                    .get("call_id")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("tool_call");
                let name = item
                    .get("name")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("tool");
                let input = runtime_anthropic_tool_input_from_arguments(
                    item.get("arguments")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("{}"),
                );
                content.push(
                    runtime_anthropic_server_tool_use_block(
                        call_id,
                        name,
                        input.clone(),
                        server_tools,
                    )
                    .unwrap_or_else(|| {
                        serde_json::json!({
                            "type": "tool_use",
                            "id": call_id,
                            "name": name,
                            "input": input,
                        })
                    }),
                );
            }
            _ => {}
        }
    }

    if content.is_empty() {
        content.push(serde_json::json!({
            "type": "text",
            "text": "",
        }));
    }

    (content, has_tool_calls)
}

#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn runtime_anthropic_response_from_json_value(
    value: &serde_json::Value,
    requested_model: &str,
    want_thinking: bool,
) -> serde_json::Value {
    runtime_anthropic_response_from_json_value_with_carried_usage(
        value,
        requested_model,
        want_thinking,
        0,
        0,
        0,
        0,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn runtime_anthropic_response_from_json_value_with_carried_usage(
    value: &serde_json::Value,
    requested_model: &str,
    want_thinking: bool,
    carried_web_search_requests: u64,
    carried_web_fetch_requests: u64,
    carried_code_execution_requests: u64,
    carried_tool_search_requests: u64,
    server_tools: Option<&RuntimeAnthropicServerTools>,
) -> serde_json::Value {
    let (input_tokens, output_tokens, cached_tokens) = runtime_anthropic_usage_from_value(value);
    let output = value
        .get("output")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();
    let web_search_requests = runtime_anthropic_tool_usage_web_search_requests_from_value(value)
        .max(runtime_anthropic_web_search_request_count_from_output(
            &output,
            server_tools,
        ))
        .max(carried_web_search_requests);
    let web_fetch_requests =
        runtime_anthropic_web_fetch_request_count_from_output(&output, server_tools)
            .max(carried_web_fetch_requests);
    let code_execution_requests =
        runtime_anthropic_tool_usage_code_execution_requests_from_value(value)
            .max(runtime_anthropic_code_execution_request_count_from_output(
                &output,
                server_tools,
            ))
            .max(carried_code_execution_requests);
    let tool_search_requests = runtime_anthropic_tool_usage_tool_search_requests_from_value(value)
        .max(runtime_anthropic_tool_search_request_count_from_output(
            &output,
            server_tools,
        ))
        .max(carried_tool_search_requests);
    let (content, has_tool_calls) =
        runtime_anthropic_output_blocks_from_json(&output, want_thinking, server_tools);
    let usage = runtime_anthropic_usage_json(
        input_tokens,
        output_tokens,
        cached_tokens,
        web_search_requests,
        web_fetch_requests,
        code_execution_requests,
        tool_search_requests,
    );
    serde_json::json!({
        "id": runtime_anthropic_message_id(),
        "type": "message",
        "role": "assistant",
        "content": content,
        "model": requested_model,
        "stop_reason": if has_tool_calls { "tool_use" } else { "end_turn" },
        "stop_sequence": serde_json::Value::Null,
        "usage": usage,
    })
}

#[derive(Debug, Clone, Default)]
pub(super) struct RuntimeAnthropicCollectedToolUse {
    call_id: String,
    name: String,
    arguments: String,
    saw_delta: bool,
    server_tool_name: Option<String>,
    server_tool_block_type: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub(super) struct RuntimeAnthropicCollectedResponse {
    content: Vec<serde_json::Value>,
    pending_text: String,
    pending_thinking: String,
    active_tool_use: Option<RuntimeAnthropicCollectedToolUse>,
    final_output: Option<Vec<serde_json::Value>>,
    input_tokens: u64,
    output_tokens: u64,
    cached_tokens: Option<u64>,
    web_search_requests: u64,
    web_fetch_requests: u64,
    code_execution_requests: u64,
    tool_search_requests: u64,
    has_tool_calls: bool,
    want_thinking: bool,
    server_tools: RuntimeAnthropicServerTools,
}

impl RuntimeAnthropicCollectedResponse {
    fn flush_text(&mut self) {
        if self.pending_text.is_empty() {
            return;
        }
        self.content.push(serde_json::json!({
            "type": "text",
            "text": std::mem::take(&mut self.pending_text),
        }));
    }

    fn flush_thinking(&mut self) {
        if self.pending_thinking.is_empty() {
            return;
        }
        self.content.push(serde_json::json!({
            "type": "thinking",
            "thinking": std::mem::take(&mut self.pending_thinking),
        }));
    }

    fn flush_pending_textual_content(&mut self) {
        self.flush_thinking();
        self.flush_text();
    }

    fn close_active_tool_use(&mut self) {
        let Some(active_tool_use) = self.active_tool_use.take() else {
            return;
        };
        self.has_tool_calls = true;
        let input = runtime_anthropic_tool_input_from_arguments(&active_tool_use.arguments);
        self.content.push(
            active_tool_use
                .server_tool_name
                .as_deref()
                .and_then(|server_tool_name| {
                    runtime_anthropic_server_tool_use_block(
                        &active_tool_use.call_id,
                        server_tool_name,
                        input.clone(),
                        Some(&self.server_tools),
                    )
                })
                .unwrap_or_else(|| {
                    serde_json::json!({
                        "type": "tool_use",
                        "id": active_tool_use.call_id,
                        "name": active_tool_use.name,
                        "input": input,
                    })
                }),
        );
    }

    fn observe_event(&mut self, value: &serde_json::Value) -> Result<()> {
        match value.get("type").and_then(serde_json::Value::as_str) {
            Some("response.reasoning_summary_text.delta") if self.want_thinking => {
                self.flush_text();
                if let Some(delta) = value.get("delta").and_then(serde_json::Value::as_str) {
                    self.pending_thinking.push_str(delta);
                }
            }
            Some("response.output_text.delta") => {
                self.flush_thinking();
                if let Some(delta) = value.get("delta").and_then(serde_json::Value::as_str) {
                    self.pending_text.push_str(delta);
                }
            }
            Some("response.output_item.added") => {
                if value
                    .get("item")
                    .and_then(|item| item.get("type"))
                    .and_then(serde_json::Value::as_str)
                    == Some("function_call")
                {
                    self.flush_pending_textual_content();
                    self.active_tool_use = Some(RuntimeAnthropicCollectedToolUse {
                        call_id: value
                            .get("item")
                            .and_then(|item| item.get("call_id"))
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or("tool_call")
                            .to_string(),
                        name: value
                            .get("item")
                            .and_then(|item| item.get("name"))
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or("tool")
                            .to_string(),
                        server_tool_name: value
                            .get("item")
                            .and_then(|item| item.get("name"))
                            .and_then(serde_json::Value::as_str)
                            .and_then(|name| {
                                runtime_anthropic_server_tool_registration_for_call(
                                    name,
                                    Some(&self.server_tools),
                                )
                            })
                            .map(|(server_tool_name, _)| server_tool_name),
                        server_tool_block_type: value
                            .get("item")
                            .and_then(|item| item.get("name"))
                            .and_then(serde_json::Value::as_str)
                            .and_then(|name| {
                                runtime_anthropic_server_tool_registration_for_call(
                                    name,
                                    Some(&self.server_tools),
                                )
                            })
                            .map(|(_, block_type)| block_type),
                        ..RuntimeAnthropicCollectedToolUse::default()
                    });
                }
            }
            Some("response.function_call_arguments.delta") => {
                if let Some(active_tool_use) = self.active_tool_use.as_mut()
                    && let Some(delta) = value.get("delta").and_then(serde_json::Value::as_str)
                {
                    active_tool_use.saw_delta = true;
                    active_tool_use.arguments.push_str(delta);
                }
            }
            Some("response.function_call_arguments.done") => {
                if let Some(active_tool_use) = self.active_tool_use.as_mut()
                    && let Some(arguments) =
                        value.get("arguments").and_then(serde_json::Value::as_str)
                    && !active_tool_use.saw_delta
                {
                    active_tool_use.arguments = arguments.to_string();
                }
            }
            Some("response.output_item.done") => {
                match value
                    .get("item")
                    .and_then(|item| item.get("type"))
                    .and_then(serde_json::Value::as_str)
                {
                    Some("function_call") => {
                        if let Some(item) = value.get("item") {
                            let usage = runtime_anthropic_output_item_server_tool_usage(
                                item,
                                Some(&self.server_tools),
                            );
                            self.web_search_requests += usage.web_search_requests;
                            self.web_fetch_requests += usage.web_fetch_requests;
                            self.code_execution_requests += usage.code_execution_requests;
                            self.tool_search_requests += usage.tool_search_requests;
                        }
                        if let Some(active_tool_use) = self.active_tool_use.as_mut() {
                            if let Some(arguments) = value
                                .get("item")
                                .and_then(|item| item.get("arguments"))
                                .and_then(serde_json::Value::as_str)
                                && !active_tool_use.saw_delta
                            {
                                active_tool_use.arguments = arguments.to_string();
                            }
                            if let Some(name) = value
                                .get("item")
                                .and_then(|item| item.get("name"))
                                .and_then(serde_json::Value::as_str)
                            {
                                active_tool_use.name = name.to_string();
                                if let Some((server_tool_name, block_type)) =
                                    runtime_anthropic_server_tool_registration_for_call(
                                        name,
                                        Some(&self.server_tools),
                                    )
                                {
                                    active_tool_use.server_tool_name = Some(server_tool_name);
                                    active_tool_use.server_tool_block_type = Some(block_type);
                                } else {
                                    active_tool_use.server_tool_name = None;
                                    active_tool_use.server_tool_block_type = None;
                                }
                            }
                        }
                        self.close_active_tool_use();
                    }
                    Some("web_search_call") => {
                        if let Some(item) = value.get("item") {
                            self.flush_pending_textual_content();
                            self.web_search_requests +=
                                runtime_anthropic_web_search_request_count_from_output_item(item);
                            self.content.extend(
                                runtime_anthropic_web_search_blocks_from_output_item(
                                    item,
                                    &BTreeMap::new(),
                                ),
                            );
                        }
                    }
                    Some("shell_call") => {
                        if let Some(item) = value.get("item") {
                            self.flush_pending_textual_content();
                            self.has_tool_calls = true;
                            self.content.push(
                                runtime_anthropic_shell_tool_use_block_from_output_item(item),
                            );
                        }
                    }
                    Some("computer_call") => {
                        if let Some(item) = value.get("item") {
                            self.flush_pending_textual_content();
                            self.has_tool_calls = true;
                            self.content.push(
                                runtime_anthropic_computer_tool_use_block_from_output_item(item),
                            );
                        }
                    }
                    _ => {}
                }
            }
            Some("response.completed") => {
                let (input_tokens, output_tokens, cached_tokens) =
                    runtime_anthropic_usage_from_value(value);
                self.input_tokens = input_tokens;
                self.output_tokens = output_tokens;
                self.cached_tokens = cached_tokens;
                self.web_search_requests = self.web_search_requests.max(
                    runtime_anthropic_tool_usage_web_search_requests_from_value(value),
                );
                self.code_execution_requests = self
                    .code_execution_requests
                    .max(runtime_anthropic_tool_usage_code_execution_requests_from_value(value));
                self.tool_search_requests = self
                    .tool_search_requests
                    .max(runtime_anthropic_tool_usage_tool_search_requests_from_value(value));
                self.final_output = value
                    .get("response")
                    .and_then(|response| response.get("output"))
                    .and_then(serde_json::Value::as_array)
                    .cloned();
                if let Some(output) = self.final_output.as_ref() {
                    self.web_search_requests = self.web_search_requests.max(
                        runtime_anthropic_web_search_request_count_from_output(
                            output,
                            Some(&self.server_tools),
                        ),
                    );
                    self.web_fetch_requests = self.web_fetch_requests.max(
                        runtime_anthropic_web_fetch_request_count_from_output(
                            output,
                            Some(&self.server_tools),
                        ),
                    );
                    self.code_execution_requests = self.code_execution_requests.max(
                        runtime_anthropic_code_execution_request_count_from_output(
                            output,
                            Some(&self.server_tools),
                        ),
                    );
                    self.tool_search_requests = self.tool_search_requests.max(
                        runtime_anthropic_tool_search_request_count_from_output(
                            output,
                            Some(&self.server_tools),
                        ),
                    );
                }
            }
            Some("error" | "response.failed") => {
                let message = value
                    .get("error")
                    .and_then(|error| error.get("message"))
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("Codex returned an error.");
                bail!(message.to_string());
            }
            _ => {}
        }
        Ok(())
    }

    fn into_response(mut self, requested_model: &str) -> serde_json::Value {
        if let Some(output) = self.final_output.take().filter(|output| !output.is_empty()) {
            let (content, has_tool_calls) = runtime_anthropic_output_blocks_from_json(
                &output,
                self.want_thinking,
                Some(&self.server_tools),
            );
            self.content = content;
            self.has_tool_calls = has_tool_calls;
        } else {
            self.close_active_tool_use();
            self.flush_pending_textual_content();
            if self.content.is_empty() {
                self.content.push(serde_json::json!({
                    "type": "text",
                    "text": "",
                }));
            }
        }
        let usage = runtime_anthropic_usage_json(
            self.input_tokens,
            self.output_tokens,
            self.cached_tokens,
            self.web_search_requests,
            self.web_fetch_requests,
            self.code_execution_requests,
            self.tool_search_requests,
        );
        serde_json::json!({
            "id": runtime_anthropic_message_id(),
            "type": "message",
            "role": "assistant",
            "content": self.content,
            "model": requested_model,
            "stop_reason": if self.has_tool_calls { "tool_use" } else { "end_turn" },
            "stop_sequence": serde_json::Value::Null,
            "usage": usage,
        })
    }
}

#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn runtime_anthropic_response_from_sse_bytes(
    body: &[u8],
    requested_model: &str,
    want_thinking: bool,
) -> Result<serde_json::Value> {
    runtime_anthropic_response_from_sse_bytes_with_carried_usage(
        body,
        requested_model,
        want_thinking,
        0,
        0,
        0,
        0,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn runtime_anthropic_response_from_sse_bytes_with_carried_usage(
    body: &[u8],
    requested_model: &str,
    want_thinking: bool,
    carried_web_search_requests: u64,
    carried_web_fetch_requests: u64,
    carried_code_execution_requests: u64,
    carried_tool_search_requests: u64,
    server_tools: Option<&RuntimeAnthropicServerTools>,
) -> Result<serde_json::Value> {
    let mut collected = RuntimeAnthropicCollectedResponse {
        want_thinking,
        web_search_requests: carried_web_search_requests,
        web_fetch_requests: carried_web_fetch_requests,
        code_execution_requests: carried_code_execution_requests,
        tool_search_requests: carried_tool_search_requests,
        server_tools: server_tools.cloned().unwrap_or_default(),
        ..RuntimeAnthropicCollectedResponse::default()
    };
    let mut line = Vec::new();
    let mut data_lines = Vec::new();

    let mut process_event = |data_lines: &mut Vec<String>| -> Result<()> {
        if data_lines.is_empty() {
            return Ok(());
        }
        let payload = data_lines.join("\n");
        let value = serde_json::from_str::<serde_json::Value>(&payload)
            .context("failed to parse buffered Responses SSE payload")?;
        collected.observe_event(&value)?;
        data_lines.clear();
        Ok(())
    };

    for byte in body {
        line.push(*byte);
        if *byte != b'\n' {
            continue;
        }
        let line_text = String::from_utf8_lossy(&line);
        let trimmed = line_text.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            process_event(&mut data_lines)?;
            line.clear();
            continue;
        }
        if let Some(payload) = trimmed.strip_prefix("data:") {
            data_lines.push(payload.trim_start().to_string());
        }
        line.clear();
    }
    if !line.is_empty() {
        let line_text = String::from_utf8_lossy(&line);
        let trimmed = line_text.trim_end_matches(['\r', '\n']);
        if let Some(payload) = trimmed.strip_prefix("data:") {
            data_lines.push(payload.trim_start().to_string());
        }
    }
    process_event(&mut data_lines)?;
    Ok(collected.into_response(requested_model))
}

pub(super) fn runtime_anthropic_json_response_parts(
    value: serde_json::Value,
) -> RuntimeBufferedResponseParts {
    RuntimeBufferedResponseParts {
        status: 200,
        headers: vec![("Content-Type".to_string(), b"application/json".to_vec())],
        body: serde_json::to_vec(&value).unwrap_or_else(|_| b"{}".to_vec()),
    }
}

pub(super) fn runtime_anthropic_sse_event_bytes(
    event_type: &str,
    data: serde_json::Value,
) -> Vec<u8> {
    format!(
        "event: {event_type}\ndata: {}\n\n",
        serde_json::to_string(&data).unwrap_or_else(|_| "{}".to_string())
    )
    .into_bytes()
}

pub(super) fn runtime_anthropic_sse_response_parts_from_message_value(
    value: serde_json::Value,
) -> RuntimeBufferedResponseParts {
    let mut body = Vec::new();
    let message_id = value
        .get("id")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("msg_prodex")
        .to_string();
    let model = value
        .get("model")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("claude-sonnet-4-6")
        .to_string();
    let stop_reason = value
        .get("stop_reason")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let stop_sequence = value
        .get("stop_sequence")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let usage = value
        .get("usage")
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));
    let message_start_usage = serde_json::json!({
        "input_tokens": 0,
        "output_tokens": 0,
        "server_tool_use": usage
            .get("server_tool_use")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({
                "web_search_requests": 0,
                "web_fetch_requests": 0,
                "code_execution_requests": 0,
                "tool_search_requests": 0,
            })),
    });

    body.extend(runtime_anthropic_sse_event_bytes(
        "message_start",
        serde_json::json!({
            "type": "message_start",
            "message": {
                "id": message_id,
                "type": "message",
                "role": "assistant",
                "content": [],
                "model": model,
                "stop_reason": serde_json::Value::Null,
                "stop_sequence": serde_json::Value::Null,
                "usage": message_start_usage,
            }
        }),
    ));

    for (index, block) in value
        .get("content")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .enumerate()
    {
        let index_value = serde_json::Value::Number((index as u64).into());
        match block.get("type").and_then(serde_json::Value::as_str) {
            Some("thinking") => {
                body.extend(runtime_anthropic_sse_event_bytes(
                    "content_block_start",
                    serde_json::json!({
                        "type": "content_block_start",
                        "index": index_value,
                        "content_block": {
                            "type": "thinking",
                            "thinking": "",
                        }
                    }),
                ));
                body.extend(runtime_anthropic_sse_event_bytes(
                    "content_block_delta",
                    serde_json::json!({
                        "type": "content_block_delta",
                        "index": index,
                        "delta": {
                            "type": "thinking_delta",
                            "thinking": block
                                .get("thinking")
                                .and_then(serde_json::Value::as_str)
                                .unwrap_or(""),
                        }
                    }),
                ));
            }
            Some("tool_use") => {
                let input_json = block
                    .get("input")
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!({}));
                body.extend(runtime_anthropic_sse_event_bytes(
                    "content_block_start",
                    serde_json::json!({
                        "type": "content_block_start",
                        "index": index_value,
                        "content_block": {
                            "type": "tool_use",
                            "id": block.get("id").cloned().unwrap_or(serde_json::Value::String("tool_use".to_string())),
                            "name": block.get("name").cloned().unwrap_or(serde_json::Value::String("tool".to_string())),
                            "input": serde_json::json!({}),
                        }
                    }),
                ));
                body.extend(runtime_anthropic_sse_event_bytes(
                    "content_block_delta",
                    serde_json::json!({
                        "type": "content_block_delta",
                        "index": index,
                        "delta": {
                            "type": "input_json_delta",
                            "partial_json": serde_json::to_string(&input_json)
                                .unwrap_or_else(|_| "{}".to_string()),
                        }
                    }),
                ));
            }
            Some("server_tool_use") => {
                let input_json = block
                    .get("input")
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!({}));
                body.extend(runtime_anthropic_sse_event_bytes(
                    "content_block_start",
                    serde_json::json!({
                        "type": "content_block_start",
                        "index": index_value,
                        "content_block": {
                            "type": "server_tool_use",
                            "id": block.get("id").cloned().unwrap_or(serde_json::Value::String("server_tool_use".to_string())),
                            "name": block.get("name").cloned().unwrap_or(serde_json::Value::String("web_search".to_string())),
                            "input": serde_json::json!({}),
                        }
                    }),
                ));
                body.extend(runtime_anthropic_sse_event_bytes(
                    "content_block_delta",
                    serde_json::json!({
                        "type": "content_block_delta",
                        "index": index,
                        "delta": {
                            "type": "input_json_delta",
                            "partial_json": serde_json::to_string(&input_json)
                                .unwrap_or_else(|_| "{}".to_string()),
                        }
                    }),
                ));
            }
            Some("mcp_tool_use") => {
                let input_json = block
                    .get("input")
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!({}));
                body.extend(runtime_anthropic_sse_event_bytes(
                    "content_block_start",
                    serde_json::json!({
                        "type": "content_block_start",
                        "index": index_value,
                        "content_block": {
                            "type": "mcp_tool_use",
                            "id": block.get("id").cloned().unwrap_or(serde_json::Value::String("mcp_tool_use".to_string())),
                            "name": block.get("name").cloned().unwrap_or(serde_json::Value::String("mcp_tool".to_string())),
                            "server_name": block.get("server_name").cloned().unwrap_or_else(|| serde_json::Value::String("mcp".to_string())),
                            "input": serde_json::json!({}),
                        }
                    }),
                ));
                body.extend(runtime_anthropic_sse_event_bytes(
                    "content_block_delta",
                    serde_json::json!({
                        "type": "content_block_delta",
                        "index": index,
                        "delta": {
                            "type": "input_json_delta",
                            "partial_json": serde_json::to_string(&input_json)
                                .unwrap_or_else(|_| "{}".to_string()),
                        }
                    }),
                ));
            }
            Some(block_type) if block_type.ends_with("_tool_result") => {
                body.extend(runtime_anthropic_sse_event_bytes(
                    "content_block_start",
                    serde_json::json!({
                        "type": "content_block_start",
                        "index": index_value,
                        "content_block": {
                            "type": block_type,
                            "tool_use_id": block.get("tool_use_id").cloned().unwrap_or_else(|| {
                                serde_json::Value::String(format!("{block_type}_call"))
                            }),
                            "content": block.get("content").cloned().unwrap_or(serde_json::Value::Null),
                        }
                    }),
                ));
            }
            Some("mcp_approval_request" | "mcp_list_tools") => {
                body.extend(runtime_anthropic_sse_event_bytes(
                    "content_block_start",
                    serde_json::json!({
                        "type": "content_block_start",
                        "index": index_value,
                        "content_block": block.clone(),
                    }),
                ));
            }
            _ => {
                body.extend(runtime_anthropic_sse_event_bytes(
                    "content_block_start",
                    serde_json::json!({
                        "type": "content_block_start",
                        "index": index_value,
                        "content_block": {
                            "type": "text",
                            "text": "",
                        }
                    }),
                ));
                body.extend(runtime_anthropic_sse_event_bytes(
                    "content_block_delta",
                    serde_json::json!({
                        "type": "content_block_delta",
                        "index": index,
                        "delta": {
                            "type": "text_delta",
                            "text": block.get("text").and_then(serde_json::Value::as_str).unwrap_or(""),
                        }
                    }),
                ));
            }
        }
        body.extend(runtime_anthropic_sse_event_bytes(
            "content_block_stop",
            serde_json::json!({
                "type": "content_block_stop",
                "index": index,
            }),
        ));
    }

    body.extend(runtime_anthropic_sse_event_bytes(
        "message_delta",
        serde_json::json!({
            "type": "message_delta",
            "delta": {
                "stop_reason": stop_reason,
                "stop_sequence": stop_sequence,
            },
            "usage": usage,
        }),
    ));
    body.extend(runtime_anthropic_sse_event_bytes(
        "message_stop",
        serde_json::json!({
            "type": "message_stop",
        }),
    ));

    RuntimeBufferedResponseParts {
        status: 200,
        headers: vec![("Content-Type".to_string(), b"text/event-stream".to_vec())],
        body,
    }
}

pub(super) fn runtime_response_body_looks_like_sse(body: &[u8]) -> bool {
    let trimmed = body
        .iter()
        .copied()
        .skip_while(|byte| byte.is_ascii_whitespace());
    let prefix = trimmed.take(8).collect::<Vec<_>>();
    prefix.starts_with(b"event:") || prefix.starts_with(b"data:")
}

pub(super) fn runtime_buffered_response_ids(parts: &RuntimeBufferedResponseParts) -> Vec<String> {
    if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&parts.body) {
        return extract_runtime_response_ids_from_value(&value);
    }

    let mut response_ids = Vec::new();
    let mut line = Vec::new();
    let mut data_lines = Vec::new();
    let push_data_lines = |data_lines: &mut Vec<String>, response_ids: &mut Vec<String>| {
        if let Some(value) = parse_runtime_sse_payload(data_lines) {
            for response_id in extract_runtime_response_ids_from_value(&value) {
                push_runtime_response_id(response_ids, Some(&response_id));
            }
        }
        data_lines.clear();
    };

    for byte in &parts.body {
        line.push(*byte);
        if *byte != b'\n' {
            continue;
        }
        let line_text = String::from_utf8_lossy(&line);
        let trimmed = line_text.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            push_data_lines(&mut data_lines, &mut response_ids);
            line.clear();
            continue;
        }
        if let Some(payload) = trimmed.strip_prefix("data:") {
            data_lines.push(payload.trim_start().to_string());
        }
        line.clear();
    }
    if !line.is_empty() {
        let line_text = String::from_utf8_lossy(&line);
        let trimmed = line_text.trim_end_matches(['\r', '\n']);
        if let Some(payload) = trimmed.strip_prefix("data:") {
            data_lines.push(payload.trim_start().to_string());
        }
    }
    push_data_lines(&mut data_lines, &mut response_ids);
    response_ids
}

pub(super) fn runtime_request_for_anthropic_server_tool_followup(
    request: &RuntimeProxyRequest,
    previous_response_id: &str,
) -> Result<RuntimeProxyRequest> {
    let mut value = serde_json::from_slice::<serde_json::Value>(&request.body)
        .context("failed to parse translated Anthropic request body")?;
    let object = value
        .as_object_mut()
        .context("translated Anthropic request body must be a JSON object")?;
    object.remove("input");
    object.remove("tool_choice");
    object.insert(
        "previous_response_id".to_string(),
        serde_json::Value::String(previous_response_id.to_string()),
    );
    object.insert("stream".to_string(), serde_json::Value::Bool(false));
    let body = serde_json::to_vec(&value)
        .context("failed to serialize Anthropic server-tool follow-up request")?;
    Ok(RuntimeProxyRequest {
        method: request.method.clone(),
        path_and_query: request.path_and_query.clone(),
        headers: request.headers.clone(),
        body,
    })
}

pub(super) fn runtime_anthropic_message_needs_server_tool_followup(
    value: &serde_json::Value,
) -> bool {
    let Some(content) = value.get("content").and_then(serde_json::Value::as_array) else {
        return false;
    };

    let mut saw_server_tool_use = false;
    for block in content {
        match block.get("type").and_then(serde_json::Value::as_str) {
            Some("server_tool_use") => saw_server_tool_use = true,
            Some("web_search_tool_result") => {}
            _ => return false,
        }
    }

    saw_server_tool_use
}

pub(super) fn runtime_anthropic_message_server_tool_usage(
    value: &serde_json::Value,
) -> RuntimeAnthropicServerToolUsage {
    let usage = value
        .get("usage")
        .and_then(|usage| usage.get("server_tool_use"));
    RuntimeAnthropicServerToolUsage {
        web_search_requests: usage
            .and_then(|server_tool_use| server_tool_use.get("web_search_requests"))
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0),
        web_fetch_requests: usage
            .and_then(|server_tool_use| server_tool_use.get("web_fetch_requests"))
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0),
        code_execution_requests: usage
            .and_then(|server_tool_use| server_tool_use.get("code_execution_requests"))
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0),
        tool_search_requests: usage
            .and_then(|server_tool_use| server_tool_use.get("tool_search_requests"))
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0),
    }
}

pub(super) fn runtime_anthropic_message_from_buffered_responses_parts_with_carried_usage(
    parts: &RuntimeBufferedResponseParts,
    request: &RuntimeAnthropicMessagesRequest,
    carried_usage: RuntimeAnthropicServerToolUsage,
) -> Result<serde_json::Value> {
    let content_type = runtime_buffered_response_content_type(parts)
        .unwrap_or_default()
        .to_ascii_lowercase();
    let looks_like_sse = content_type.contains("text/event-stream")
        || runtime_response_body_looks_like_sse(&parts.body);
    if looks_like_sse {
        return runtime_anthropic_response_from_sse_bytes_with_carried_usage(
            &parts.body,
            &request.requested_model,
            request.want_thinking,
            carried_usage.web_search_requests,
            carried_usage.web_fetch_requests,
            carried_usage.code_execution_requests,
            carried_usage.tool_search_requests,
            Some(&request.server_tools),
        );
    }

    let value = serde_json::from_slice::<serde_json::Value>(&parts.body)
        .context("failed to parse buffered Responses JSON body")?;
    Ok(
        runtime_anthropic_response_from_json_value_with_carried_usage(
            &value,
            &request.requested_model,
            request.want_thinking,
            carried_usage.web_search_requests,
            carried_usage.web_fetch_requests,
            carried_usage.code_execution_requests,
            carried_usage.tool_search_requests,
            Some(&request.server_tools),
        ),
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn runtime_anthropic_sse_response_parts_from_responses_sse_bytes(
    body: &[u8],
    requested_model: &str,
    want_thinking: bool,
    carried_web_search_requests: u64,
    carried_web_fetch_requests: u64,
    carried_code_execution_requests: u64,
    carried_tool_search_requests: u64,
    server_tools: &RuntimeAnthropicServerTools,
) -> Result<RuntimeBufferedResponseParts> {
    let response = runtime_anthropic_response_from_sse_bytes_with_carried_usage(
        body,
        requested_model,
        want_thinking,
        carried_web_search_requests,
        carried_web_fetch_requests,
        carried_code_execution_requests,
        carried_tool_search_requests,
        Some(server_tools),
    )
    .context("failed to translate buffered Responses SSE body")?;
    Ok(runtime_anthropic_sse_response_parts_from_message_value(
        response,
    ))
}

pub(super) fn buffer_runtime_streaming_response_parts(
    response: RuntimeStreamingResponse,
) -> Result<RuntimeBufferedResponseParts> {
    let RuntimeStreamingResponse {
        status,
        headers,
        mut body,
        ..
    } = response;
    let mut buffered_body = Vec::new();
    body.read_to_end(&mut buffered_body)
        .context("failed to buffer streaming runtime response")?;
    Ok(RuntimeBufferedResponseParts {
        status,
        headers: headers
            .into_iter()
            .map(|(name, value)| (name, value.into_bytes()))
            .collect(),
        body: buffered_body,
    })
}

#[derive(Debug, Clone, Default)]
pub(super) struct RuntimeAnthropicStreamToolUse {
    call_id: String,
    name: String,
    arguments: String,
    saw_delta: bool,
    server_tool_name: Option<String>,
    server_tool_block_type: Option<String>,
}

pub(super) struct RuntimeAnthropicSseReader {
    inner: Box<dyn Read + Send>,
    pending: VecDeque<u8>,
    upstream_line: Vec<u8>,
    upstream_data_lines: Vec<String>,
    message_id: String,
    model: String,
    want_thinking: bool,
    content_index: usize,
    thinking_block_open: bool,
    text_block_open: bool,
    has_tool_calls: bool,
    has_content: bool,
    input_tokens: u64,
    output_tokens: u64,
    cached_tokens: Option<u64>,
    web_search_requests: u64,
    web_fetch_requests: u64,
    code_execution_requests: u64,
    tool_search_requests: u64,
    server_tools: RuntimeAnthropicServerTools,
    active_tool_use: Option<RuntimeAnthropicStreamToolUse>,
    terminal_sent: bool,
    inner_finished: bool,
}

impl RuntimeAnthropicSseReader {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        inner: Box<dyn Read + Send>,
        model: String,
        want_thinking: bool,
        carried_web_search_requests: u64,
        carried_web_fetch_requests: u64,
        carried_code_execution_requests: u64,
        carried_tool_search_requests: u64,
        server_tools: RuntimeAnthropicServerTools,
    ) -> Self {
        let mut reader = Self {
            inner,
            pending: VecDeque::new(),
            upstream_line: Vec::new(),
            upstream_data_lines: Vec::new(),
            message_id: runtime_anthropic_message_id(),
            model,
            want_thinking,
            content_index: 0,
            thinking_block_open: false,
            text_block_open: false,
            has_tool_calls: false,
            has_content: false,
            input_tokens: 0,
            output_tokens: 0,
            cached_tokens: None,
            web_search_requests: carried_web_search_requests,
            web_fetch_requests: carried_web_fetch_requests,
            code_execution_requests: carried_code_execution_requests,
            tool_search_requests: carried_tool_search_requests,
            server_tools,
            active_tool_use: None,
            terminal_sent: false,
            inner_finished: false,
        };
        reader.push_event(
            "message_start",
            serde_json::json!({
                "type": "message_start",
                "message": {
                    "id": reader.message_id.clone(),
                    "type": "message",
                    "role": "assistant",
                    "content": [],
                    "model": reader.model.clone(),
                    "stop_reason": serde_json::Value::Null,
                    "stop_sequence": serde_json::Value::Null,
                    "usage": {
                        "input_tokens": 0,
                        "output_tokens": 0,
                        "server_tool_use": {
                            "web_search_requests": carried_web_search_requests,
                            "web_fetch_requests": carried_web_fetch_requests,
                            "code_execution_requests": carried_code_execution_requests,
                            "tool_search_requests": carried_tool_search_requests,
                        }
                    }
                }
            }),
        );
        reader
    }

    fn push_event(&mut self, event_type: &str, data: serde_json::Value) {
        let frame = format!(
            "event: {event_type}\ndata: {}\n\n",
            serde_json::to_string(&data).unwrap_or_else(|_| "{}".to_string())
        );
        self.pending.extend(frame.into_bytes());
    }

    fn close_thinking_block(&mut self) {
        if !self.thinking_block_open {
            return;
        }
        self.push_event(
            "content_block_stop",
            serde_json::json!({
                "type": "content_block_stop",
                "index": self.content_index,
            }),
        );
        self.content_index += 1;
        self.thinking_block_open = false;
    }

    fn close_text_block(&mut self) {
        if !self.text_block_open {
            return;
        }
        self.push_event(
            "content_block_stop",
            serde_json::json!({
                "type": "content_block_stop",
                "index": self.content_index,
            }),
        );
        self.content_index += 1;
        self.text_block_open = false;
    }

    fn ensure_text_block(&mut self) {
        if self.text_block_open {
            return;
        }
        self.push_event(
            "content_block_start",
            serde_json::json!({
                "type": "content_block_start",
                "index": self.content_index,
                "content_block": {
                    "type": "text",
                    "text": "",
                }
            }),
        );
        self.text_block_open = true;
    }

    fn ensure_thinking_block(&mut self) {
        if self.thinking_block_open {
            return;
        }
        self.push_event(
            "content_block_start",
            serde_json::json!({
                "type": "content_block_start",
                "index": self.content_index,
                "content_block": {
                    "type": "thinking",
                    "thinking": "",
                }
            }),
        );
        self.thinking_block_open = true;
    }

    fn start_tool_use_block(&mut self, call_id: &str, name: &str) {
        let server_tool_registration =
            runtime_anthropic_server_tool_registration_for_call(name, Some(&self.server_tools));
        let block_type = server_tool_registration
            .as_ref()
            .map(|(_, block_type)| block_type.as_str())
            .unwrap_or("tool_use");
        let output_name = server_tool_registration
            .as_ref()
            .map(|(server_tool_name, _)| server_tool_name.as_str())
            .unwrap_or(name);
        let mut content_block = serde_json::Map::new();
        content_block.insert(
            "type".to_string(),
            serde_json::Value::String(block_type.to_string()),
        );
        content_block.insert(
            "id".to_string(),
            serde_json::Value::String(call_id.to_string()),
        );
        content_block.insert(
            "name".to_string(),
            serde_json::Value::String(output_name.to_string()),
        );
        content_block.insert("input".to_string(), serde_json::json!({}));
        self.close_thinking_block();
        self.close_text_block();
        self.push_event(
            "content_block_start",
            serde_json::json!({
                "type": "content_block_start",
                "index": self.content_index,
                "content_block": serde_json::Value::Object(content_block)
            }),
        );
        self.active_tool_use = Some(RuntimeAnthropicStreamToolUse {
            call_id: call_id.to_string(),
            name: name.to_string(),
            server_tool_name: server_tool_registration
                .as_ref()
                .map(|(server_tool_name, _)| server_tool_name.clone()),
            server_tool_block_type: server_tool_registration.map(|(_, block_type)| block_type),
            ..RuntimeAnthropicStreamToolUse::default()
        });
        self.has_content = true;
        self.has_tool_calls = true;
    }

    fn finish_active_tool_use(
        &mut self,
        arguments_override: Option<&str>,
        name_override: Option<&str>,
        call_id_override: Option<&str>,
    ) {
        let Some(mut active_tool_use) = self.active_tool_use.take() else {
            return;
        };
        if let Some(name) = name_override {
            active_tool_use.name = name.to_string();
            if let Some((server_tool_name, block_type)) =
                runtime_anthropic_server_tool_registration_for_call(name, Some(&self.server_tools))
            {
                active_tool_use.server_tool_name = Some(server_tool_name);
                active_tool_use.server_tool_block_type = Some(block_type);
            } else {
                active_tool_use.server_tool_name = None;
                active_tool_use.server_tool_block_type = None;
            }
        }
        if let Some(call_id) = call_id_override {
            active_tool_use.call_id = call_id.to_string();
        }
        if let Some(arguments) = arguments_override
            && !active_tool_use.saw_delta
        {
            active_tool_use.arguments = arguments.to_string();
        }
        if !active_tool_use.saw_delta && !active_tool_use.arguments.is_empty() {
            self.push_event(
                "content_block_delta",
                serde_json::json!({
                    "type": "content_block_delta",
                    "index": self.content_index,
                    "delta": {
                        "type": "input_json_delta",
                        "partial_json": active_tool_use.arguments,
                    }
                }),
            );
        }
        self.push_event(
            "content_block_stop",
            serde_json::json!({
                "type": "content_block_stop",
                "index": self.content_index,
            }),
        );
        self.content_index += 1;
    }

    fn finish_success(&mut self) {
        if self.terminal_sent {
            return;
        }
        self.finish_active_tool_use(None, None, None);
        self.close_thinking_block();
        self.close_text_block();
        if !self.has_content {
            self.ensure_text_block();
            self.close_text_block();
        }
        let usage = runtime_anthropic_usage_json(
            self.input_tokens,
            self.output_tokens,
            self.cached_tokens,
            self.web_search_requests,
            self.web_fetch_requests,
            self.code_execution_requests,
            self.tool_search_requests,
        );
        self.push_event(
            "message_delta",
            serde_json::json!({
                "type": "message_delta",
                "delta": {
                    "stop_reason": if self.has_tool_calls { "tool_use" } else { "end_turn" },
                },
                "usage": usage,
            }),
        );
        self.push_event(
            "message_stop",
            serde_json::json!({
                "type": "message_stop",
            }),
        );
        self.terminal_sent = true;
        self.inner_finished = true;
    }

    fn finish_error(&mut self, message: &str) {
        if self.terminal_sent {
            return;
        }
        self.finish_active_tool_use(None, None, None);
        self.close_thinking_block();
        self.close_text_block();
        self.ensure_text_block();
        self.push_event(
            "content_block_delta",
            serde_json::json!({
                "type": "content_block_delta",
                "index": self.content_index,
                "delta": {
                    "type": "text_delta",
                    "text": format!("[Error] {message}"),
                }
            }),
        );
        self.has_content = true;
        self.close_text_block();
        self.push_event(
            "error",
            serde_json::json!({
                "type": "error",
                "error": {
                    "type": "api_error",
                    "message": message,
                }
            }),
        );
        self.push_event(
            "message_stop",
            serde_json::json!({
                "type": "message_stop",
            }),
        );
        self.terminal_sent = true;
        self.inner_finished = true;
    }

    fn observe_upstream_event(&mut self, value: &serde_json::Value) {
        match value.get("type").and_then(serde_json::Value::as_str) {
            Some("response.reasoning_summary_text.delta") if self.want_thinking => {
                self.close_text_block();
                self.ensure_thinking_block();
                if let Some(delta) = value.get("delta").and_then(serde_json::Value::as_str) {
                    self.push_event(
                        "content_block_delta",
                        serde_json::json!({
                            "type": "content_block_delta",
                            "index": self.content_index,
                            "delta": {
                                "type": "thinking_delta",
                                "thinking": delta,
                            }
                        }),
                    );
                    self.has_content = true;
                }
            }
            Some("response.output_text.delta") => {
                self.close_thinking_block();
                self.ensure_text_block();
                if let Some(delta) = value.get("delta").and_then(serde_json::Value::as_str) {
                    self.push_event(
                        "content_block_delta",
                        serde_json::json!({
                            "type": "content_block_delta",
                            "index": self.content_index,
                            "delta": {
                                "type": "text_delta",
                                "text": delta,
                            }
                        }),
                    );
                    self.has_content = true;
                }
            }
            Some("response.output_item.added") => {
                if value
                    .get("item")
                    .and_then(|item| item.get("type"))
                    .and_then(serde_json::Value::as_str)
                    == Some("function_call")
                {
                    let call_id = value
                        .get("item")
                        .and_then(|item| item.get("call_id"))
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("tool_call");
                    let name = value
                        .get("item")
                        .and_then(|item| item.get("name"))
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("tool");
                    self.start_tool_use_block(call_id, name);
                } else if value
                    .get("item")
                    .and_then(|item| item.get("type"))
                    .and_then(serde_json::Value::as_str)
                    == Some("shell_call")
                {
                    let call_id = value
                        .get("item")
                        .and_then(|item| item.get("call_id"))
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("shell_call");
                    self.start_tool_use_block(call_id, "bash");
                } else if value
                    .get("item")
                    .and_then(|item| item.get("type"))
                    .and_then(serde_json::Value::as_str)
                    == Some("computer_call")
                {
                    let call_id = value
                        .get("item")
                        .and_then(|item| item.get("call_id"))
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("computer_call");
                    self.start_tool_use_block(call_id, "computer");
                }
            }
            Some("response.function_call_arguments.delta") => {
                if let Some(delta) = value.get("delta").and_then(serde_json::Value::as_str) {
                    if let Some(active_tool_use) = self.active_tool_use.as_mut() {
                        active_tool_use.saw_delta = true;
                        active_tool_use.arguments.push_str(delta);
                    }
                    self.push_event(
                        "content_block_delta",
                        serde_json::json!({
                            "type": "content_block_delta",
                            "index": self.content_index,
                            "delta": {
                                "type": "input_json_delta",
                                "partial_json": delta,
                            }
                        }),
                    );
                }
            }
            Some("response.function_call_arguments.done") => {
                if let Some(active_tool_use) = self.active_tool_use.as_mut()
                    && let Some(arguments) =
                        value.get("arguments").and_then(serde_json::Value::as_str)
                    && !active_tool_use.saw_delta
                {
                    active_tool_use.arguments = arguments.to_string();
                }
            }
            Some("response.output_item.done") => {
                match value
                    .get("item")
                    .and_then(|item| item.get("type"))
                    .and_then(serde_json::Value::as_str)
                {
                    Some("function_call") => {
                        if let Some(item) = value.get("item") {
                            let usage = runtime_anthropic_output_item_server_tool_usage(
                                item,
                                Some(&self.server_tools),
                            );
                            self.web_search_requests += usage.web_search_requests;
                            self.web_fetch_requests += usage.web_fetch_requests;
                            self.code_execution_requests += usage.code_execution_requests;
                            self.tool_search_requests += usage.tool_search_requests;
                        }
                        if self.active_tool_use.is_none() {
                            let call_id = value
                                .get("item")
                                .and_then(|item| item.get("call_id"))
                                .and_then(serde_json::Value::as_str)
                                .unwrap_or("tool_call");
                            let name = value
                                .get("item")
                                .and_then(|item| item.get("name"))
                                .and_then(serde_json::Value::as_str)
                                .unwrap_or("tool");
                            self.start_tool_use_block(call_id, name);
                        }
                        let arguments = value
                            .get("item")
                            .and_then(|item| item.get("arguments"))
                            .and_then(serde_json::Value::as_str);
                        let name = value
                            .get("item")
                            .and_then(|item| item.get("name"))
                            .and_then(serde_json::Value::as_str);
                        let call_id = value
                            .get("item")
                            .and_then(|item| item.get("call_id"))
                            .and_then(serde_json::Value::as_str);
                        self.finish_active_tool_use(arguments, name, call_id);
                    }
                    Some("web_search_call") => {
                        if let Some(item) = value.get("item") {
                            self.web_search_requests +=
                                runtime_anthropic_web_search_request_count_from_output_item(item);
                        }
                    }
                    Some("shell_call") => {
                        if self.active_tool_use.is_none() {
                            let call_id = value
                                .get("item")
                                .and_then(|item| item.get("call_id"))
                                .and_then(serde_json::Value::as_str)
                                .unwrap_or("shell_call");
                            self.start_tool_use_block(call_id, "bash");
                        }
                        let call_id = value
                            .get("item")
                            .and_then(|item| item.get("call_id"))
                            .and_then(serde_json::Value::as_str);
                        let arguments = value
                            .get("item")
                            .map(runtime_anthropic_shell_tool_input_from_output_item)
                            .and_then(|input| serde_json::to_string(&input).ok());
                        self.finish_active_tool_use(arguments.as_deref(), Some("bash"), call_id);
                    }
                    Some("computer_call") => {
                        if self.active_tool_use.is_none() {
                            let call_id = value
                                .get("item")
                                .and_then(|item| item.get("call_id"))
                                .and_then(serde_json::Value::as_str)
                                .unwrap_or("computer_call");
                            self.start_tool_use_block(call_id, "computer");
                        }
                        let call_id = value
                            .get("item")
                            .and_then(|item| item.get("call_id"))
                            .and_then(serde_json::Value::as_str);
                        let arguments = value
                            .get("item")
                            .map(runtime_anthropic_computer_tool_input_from_output_item)
                            .map(|input| {
                                input.unwrap_or_else(|| {
                                    runtime_anthropic_raw_computer_tool_input_from_output_item(
                                        value.get("item").unwrap_or(&serde_json::Value::Null),
                                    )
                                })
                            })
                            .and_then(|input| serde_json::to_string(&input).ok());
                        self.finish_active_tool_use(
                            arguments.as_deref(),
                            Some("computer"),
                            call_id,
                        );
                    }
                    _ => {}
                }
            }
            Some("response.completed") => {
                let (input_tokens, output_tokens, cached_tokens) =
                    runtime_anthropic_usage_from_value(value);
                self.input_tokens = input_tokens;
                self.output_tokens = output_tokens;
                self.cached_tokens = cached_tokens;
                self.web_search_requests = self.web_search_requests.max(
                    runtime_anthropic_tool_usage_web_search_requests_from_value(value),
                );
                self.code_execution_requests = self
                    .code_execution_requests
                    .max(runtime_anthropic_tool_usage_code_execution_requests_from_value(value));
                self.tool_search_requests = self
                    .tool_search_requests
                    .max(runtime_anthropic_tool_usage_tool_search_requests_from_value(value));
                if let Some(output) = value
                    .get("response")
                    .and_then(|response| response.get("output"))
                    .and_then(serde_json::Value::as_array)
                {
                    self.web_search_requests = self.web_search_requests.max(
                        runtime_anthropic_web_search_request_count_from_output(
                            output,
                            Some(&self.server_tools),
                        ),
                    );
                    self.web_fetch_requests = self.web_fetch_requests.max(
                        runtime_anthropic_web_fetch_request_count_from_output(
                            output,
                            Some(&self.server_tools),
                        ),
                    );
                    self.code_execution_requests = self.code_execution_requests.max(
                        runtime_anthropic_code_execution_request_count_from_output(
                            output,
                            Some(&self.server_tools),
                        ),
                    );
                    self.tool_search_requests = self.tool_search_requests.max(
                        runtime_anthropic_tool_search_request_count_from_output(
                            output,
                            Some(&self.server_tools),
                        ),
                    );
                }
                self.finish_success();
            }
            Some("error" | "response.failed") => {
                let message = value
                    .get("error")
                    .and_then(|error| error.get("message"))
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("Codex returned an error.");
                self.finish_error(message);
            }
            _ => {}
        }
    }

    fn process_upstream_event(&mut self) -> io::Result<()> {
        if self.upstream_data_lines.is_empty() {
            return Ok(());
        }
        let payload = self.upstream_data_lines.join("\n");
        self.upstream_data_lines.clear();
        let value = serde_json::from_str::<serde_json::Value>(&payload).map_err(|err| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("failed to parse runtime Responses SSE payload: {err}"),
            )
        })?;
        self.observe_upstream_event(&value);
        Ok(())
    }

    fn observe_upstream_bytes(&mut self, chunk: &[u8]) -> io::Result<()> {
        for byte in chunk {
            self.upstream_line.push(*byte);
            if *byte != b'\n' {
                continue;
            }
            let line_text = String::from_utf8_lossy(&self.upstream_line);
            let trimmed = line_text.trim_end_matches(['\r', '\n']);
            if trimmed.is_empty() {
                self.process_upstream_event()?;
                self.upstream_line.clear();
                if self.inner_finished {
                    break;
                }
                continue;
            }
            if let Some(payload) = trimmed.strip_prefix("data:") {
                self.upstream_data_lines
                    .push(payload.trim_start().to_string());
            }
            self.upstream_line.clear();
        }
        Ok(())
    }
}

impl Read for RuntimeAnthropicSseReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        loop {
            let read = buf.len().min(self.pending.len());
            if read > 0 {
                for (index, byte) in self.pending.drain(..read).enumerate() {
                    buf[index] = byte;
                }
                return Ok(read);
            }
            if self.inner_finished {
                return Ok(0);
            }

            let mut upstream_buffer = [0_u8; 8192];
            let read = self.inner.read(&mut upstream_buffer)?;
            if read == 0 {
                self.finish_success();
                continue;
            }
            self.observe_upstream_bytes(&upstream_buffer[..read])?;
        }
    }
}

pub(super) fn translate_runtime_buffered_responses_reply_to_anthropic(
    parts: RuntimeBufferedResponseParts,
    request: &RuntimeAnthropicMessagesRequest,
) -> Result<RuntimeResponsesReply> {
    if parts.status >= 400 {
        return Ok(RuntimeResponsesReply::Buffered(
            runtime_anthropic_error_from_upstream_parts(parts),
        ));
    }

    let content_type = runtime_buffered_response_content_type(&parts)
        .unwrap_or_default()
        .to_ascii_lowercase();
    let looks_like_sse = content_type.contains("text/event-stream")
        || runtime_response_body_looks_like_sse(&parts.body);
    if request.stream && looks_like_sse {
        return Ok(RuntimeResponsesReply::Buffered(
            runtime_anthropic_sse_response_parts_from_responses_sse_bytes(
                &parts.body,
                &request.requested_model,
                request.want_thinking,
                request.carried_web_search_requests,
                request.carried_web_fetch_requests,
                request.carried_code_execution_requests,
                request.carried_tool_search_requests,
                &request.server_tools,
            )?,
        ));
    }

    let response = if looks_like_sse {
        runtime_anthropic_response_from_sse_bytes_with_carried_usage(
            &parts.body,
            &request.requested_model,
            request.want_thinking,
            request.carried_web_search_requests,
            request.carried_web_fetch_requests,
            request.carried_code_execution_requests,
            request.carried_tool_search_requests,
            Some(&request.server_tools),
        )?
    } else {
        let value = serde_json::from_slice::<serde_json::Value>(&parts.body)
            .context("failed to parse buffered Responses JSON body")?;
        if value.get("error").is_some() {
            return Ok(RuntimeResponsesReply::Buffered(
                runtime_anthropic_error_from_upstream_parts(parts),
            ));
        }
        runtime_anthropic_response_from_json_value_with_carried_usage(
            &value,
            &request.requested_model,
            request.want_thinking,
            request.carried_web_search_requests,
            request.carried_web_fetch_requests,
            request.carried_code_execution_requests,
            request.carried_tool_search_requests,
            Some(&request.server_tools),
        )
    };

    if request.stream {
        return Ok(RuntimeResponsesReply::Buffered(
            runtime_anthropic_sse_response_parts_from_message_value(response),
        ));
    }

    Ok(RuntimeResponsesReply::Buffered(
        runtime_anthropic_json_response_parts(response),
    ))
}

pub(super) fn translate_runtime_responses_reply_to_anthropic(
    response: RuntimeResponsesReply,
    request: &RuntimeAnthropicMessagesRequest,
    request_id: u64,
    shared: &RuntimeRotationProxyShared,
) -> Result<RuntimeResponsesReply> {
    if request.server_tools.needs_buffered_translation() {
        let mut parts = match response {
            RuntimeResponsesReply::Buffered(parts) => parts,
            RuntimeResponsesReply::Streaming(response) => {
                buffer_runtime_streaming_response_parts(response)?
            }
        };
        let mut carried_usage = RuntimeAnthropicServerToolUsage {
            web_search_requests: request.carried_web_search_requests,
            web_fetch_requests: request.carried_web_fetch_requests,
            code_execution_requests: request.carried_code_execution_requests,
            tool_search_requests: request.carried_tool_search_requests,
        };

        for followup_attempt in 0..=RUNTIME_PROXY_ANTHROPIC_WEB_SEARCH_FOLLOWUP_LIMIT {
            if std::env::var_os("PRODEX_DEBUG_ANTHROPIC_COMPAT").is_some() {
                runtime_proxy_log(
                    shared,
                    format!(
                        "request={request_id} transport=http anthropic_translated_upstream status={} content_type={:?} followup_attempt={} body_snippet={}",
                        parts.status,
                        runtime_buffered_response_content_type(&parts),
                        followup_attempt,
                        runtime_proxy_body_snippet(&parts.body, 2048),
                    ),
                );
            }

            if parts.status >= 400 {
                return Ok(RuntimeResponsesReply::Buffered(
                    runtime_anthropic_error_from_upstream_parts(parts),
                ));
            }

            if !runtime_response_body_looks_like_sse(&parts.body)
                && !runtime_buffered_response_content_type(&parts)
                    .unwrap_or_default()
                    .to_ascii_lowercase()
                    .contains("text/event-stream")
                && serde_json::from_slice::<serde_json::Value>(&parts.body)
                    .ok()
                    .is_some_and(|value| value.get("error").is_some())
            {
                return Ok(RuntimeResponsesReply::Buffered(
                    runtime_anthropic_error_from_upstream_parts(parts),
                ));
            }

            let response_message =
                runtime_anthropic_message_from_buffered_responses_parts_with_carried_usage(
                    &parts,
                    request,
                    carried_usage,
                )?;
            carried_usage = runtime_anthropic_message_server_tool_usage(&response_message);

            if followup_attempt == RUNTIME_PROXY_ANTHROPIC_WEB_SEARCH_FOLLOWUP_LIMIT
                || !runtime_anthropic_message_needs_server_tool_followup(&response_message)
            {
                if request.stream {
                    return Ok(RuntimeResponsesReply::Buffered(
                        runtime_anthropic_sse_response_parts_from_message_value(response_message),
                    ));
                }

                return Ok(RuntimeResponsesReply::Buffered(
                    runtime_anthropic_json_response_parts(response_message),
                ));
            }

            let Some(previous_response_id) = runtime_buffered_response_ids(&parts).last().cloned()
            else {
                if request.stream {
                    return Ok(RuntimeResponsesReply::Buffered(
                        runtime_anthropic_sse_response_parts_from_message_value(response_message),
                    ));
                }
                return Ok(RuntimeResponsesReply::Buffered(
                    runtime_anthropic_json_response_parts(response_message),
                ));
            };

            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} transport=http anthropic_server_tool_followup previous_response_id={previous_response_id} attempt={}",
                    followup_attempt + 1,
                ),
            );
            let followup_request = runtime_request_for_anthropic_server_tool_followup(
                &request.translated_request,
                &previous_response_id,
            )?;
            parts = match proxy_runtime_responses_request(request_id, &followup_request, shared)? {
                RuntimeResponsesReply::Buffered(parts) => parts,
                RuntimeResponsesReply::Streaming(response) => {
                    buffer_runtime_streaming_response_parts(response)?
                }
            };
        }

        unreachable!("anthropic buffered server-tool translation should return inside loop");
    }

    match response {
        RuntimeResponsesReply::Buffered(parts) => {
            translate_runtime_buffered_responses_reply_to_anthropic(parts, request)
        }
        RuntimeResponsesReply::Streaming(response) => {
            if !request.stream {
                let parts = buffer_runtime_streaming_response_parts(response)?;
                return translate_runtime_buffered_responses_reply_to_anthropic(parts, request);
            }

            let mut headers = response.headers;
            headers.retain(|(name, _)| !name.eq_ignore_ascii_case("content-type"));
            headers.push(("Content-Type".to_string(), "text/event-stream".to_string()));
            Ok(RuntimeResponsesReply::Streaming(RuntimeStreamingResponse {
                status: response.status,
                headers,
                body: Box::new(RuntimeAnthropicSseReader::new(
                    response.body,
                    request.requested_model.clone(),
                    request.want_thinking,
                    request.carried_web_search_requests,
                    request.carried_web_fetch_requests,
                    request.carried_code_execution_requests,
                    request.carried_tool_search_requests,
                    request.server_tools.clone(),
                )),
                request_id: response.request_id,
                profile_name: response.profile_name,
                log_path: response.log_path,
                shared: response.shared,
                _inflight_guard: response._inflight_guard,
            }))
        }
    }
}
