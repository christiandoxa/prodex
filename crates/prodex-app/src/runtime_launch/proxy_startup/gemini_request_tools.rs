use super::super::gemini_request_policy::{RuntimeGeminiPolicyCompat, runtime_gemini_tool_aliases};
use super::super::gemini_schema::runtime_gemini_sanitize_function_schema;

pub(super) fn runtime_gemini_tools_from_requests(
    original: &serde_json::Value,
    chat: &serde_json::Value,
    model: &str,
) -> Option<serde_json::Value> {
    let mut tools = Vec::new();
    if let Some(computer_use) = runtime_gemini_computer_use_tool(original) {
        tools.push(serde_json::json!({ "computerUse": computer_use }));
    }
    if runtime_gemini_code_execution_enabled(original) {
        tools.push(serde_json::json!({ "codeExecution": {} }));
    }
    if runtime_gemini_web_search_enabled(original) {
        tools.push(serde_json::json!({ "googleSearch": {} }));
    }
    if runtime_gemini_url_context_enabled(original) {
        tools.push(serde_json::json!({ "urlContext": {} }));
    }
    if let Some(serde_json::Value::Array(function_tools)) =
        runtime_gemini_function_tools_from_chat(original, chat, model)
    {
        tools.extend(function_tools);
    }
    (!tools.is_empty()).then_some(serde_json::Value::Array(tools))
}

pub(in super::super::super) fn runtime_gemini_request_body_without_tool(
    body: &[u8],
    tool_name: &str,
) -> Option<Vec<u8>> {
    let mut value: serde_json::Value = serde_json::from_slice(body).ok()?;
    let request = runtime_gemini_request_object_mut(&mut value)?;
    let tools = request.get_mut("tools")?.as_array_mut()?;
    let original_len = tools.len();
    tools.retain(|tool| {
        !tool
            .as_object()
            .map(|object| object.contains_key(tool_name))
            .unwrap_or(false)
    });
    if tools.len() == original_len {
        return None;
    }
    if tools.is_empty() {
        request.remove("tools");
    }
    serde_json::to_vec(&value).ok()
}

fn runtime_gemini_request_object_mut(
    value: &mut serde_json::Value,
) -> Option<&mut serde_json::Map<String, serde_json::Value>> {
    if value.get("request").is_some() {
        value.get_mut("request")?.as_object_mut()
    } else {
        value.as_object_mut()
    }
}

fn runtime_gemini_web_search_enabled(original: &serde_json::Value) -> bool {
    original
        .get("tools")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|tools| tools.iter().any(runtime_gemini_is_web_search_tool))
}

fn runtime_gemini_computer_use_tool(original: &serde_json::Value) -> Option<serde_json::Value> {
    let tool = original
        .get("tools")
        .and_then(serde_json::Value::as_array)?
        .iter()
        .find(|tool| runtime_gemini_is_computer_use_tool(tool))?;
    let source = tool
        .get("computerUse")
        .or_else(|| tool.get("computer_use"))
        .unwrap_or(tool);
    let environment = source
        .get("environment")
        .and_then(serde_json::Value::as_str)
        .filter(|environment| !environment.trim().is_empty())
        .unwrap_or("ENVIRONMENT_BROWSER");
    let mut computer_use = serde_json::json!({
        "environment": environment,
    });
    if let Some(excluded) = source
        .get("excludedPredefinedFunctions")
        .or_else(|| source.get("excluded_predefined_functions"))
        .filter(|value| !value.is_null())
    {
        computer_use["excludedPredefinedFunctions"] = excluded.clone();
    }
    Some(computer_use)
}

fn runtime_gemini_is_computer_use_tool(tool: &serde_json::Value) -> bool {
    let tool_type = tool
        .get("type")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    matches!(
        tool_type,
        "computer" | "computer_use" | "computerUse" | "computer_use_preview"
    ) || tool_type.starts_with("computer_")
        || tool
            .as_object()
            .is_some_and(|object| object.contains_key("computerUse"))
}

fn runtime_gemini_code_execution_enabled(original: &serde_json::Value) -> bool {
    original
        .get("tools")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|tools| tools.iter().any(runtime_gemini_is_code_execution_tool))
}

fn runtime_gemini_url_context_enabled(original: &serde_json::Value) -> bool {
    original
        .get("tools")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|tools| tools.iter().any(runtime_gemini_is_url_context_tool))
}

fn runtime_gemini_is_code_execution_tool(tool: &serde_json::Value) -> bool {
    let tool_type = tool
        .get("type")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    matches!(
        tool_type,
        "code_interpreter" | "code_execution" | "codeExecution"
    ) || tool
        .as_object()
        .is_some_and(|object| object.contains_key("codeExecution"))
}

fn runtime_gemini_is_web_search_tool(tool: &serde_json::Value) -> bool {
    let tool_type = tool
        .get("type")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    tool_type == "web_search"
        || tool_type == "web_search_preview"
        || tool_type.starts_with("web_search_preview_")
}

fn runtime_gemini_is_url_context_tool(tool: &serde_json::Value) -> bool {
    let tool_type = tool
        .get("type")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    tool_type == "web_fetch"
        || tool_type == "url_context"
        || tool_type == "urlContext"
        || tool_type == "web_fetch_preview"
        || tool_type.starts_with("web_fetch_preview_")
        || tool
            .as_object()
            .is_some_and(|object| object.contains_key("urlContext"))
}

pub(in super::super::super) fn runtime_gemini_blocked_tool_call_message(
    name: &str,
    args: &serde_json::Value,
) -> Option<String> {
    RuntimeGeminiPolicyCompat::from_request_and_files(&serde_json::Value::Null)
        .blocked_tool_call_message(name, args)
}

fn runtime_gemini_function_tools_from_chat(
    original: &serde_json::Value,
    chat: &serde_json::Value,
    model: &str,
) -> Option<serde_json::Value> {
    let mut declarations = chat
        .get("tools")?
        .as_array()?
        .iter()
        .filter_map(runtime_gemini_function_declaration_from_chat_tool)
        .collect::<Vec<_>>();
    runtime_gemini_apply_gemini3_tool_declaration_overrides(model, &mut declarations);
    RuntimeGeminiPolicyCompat::from_request_and_files(original)
        .filter_function_declarations(&mut declarations);
    (!declarations.is_empty()).then(|| {
        serde_json::json!([{
            "functionDeclarations": declarations,
        }])
    })
}

fn runtime_gemini_apply_gemini3_tool_declaration_overrides(
    model: &str,
    declarations: &mut [serde_json::Value],
) {
    if !runtime_gemini_model_uses_gemini3_toolset(model) {
        return;
    }
    for declaration in declarations {
        let Some(name) = declaration
            .get("name")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string)
        else {
            continue;
        };
        if let Some(description) = runtime_gemini_gemini3_tool_description(&name)
            && let Some(object) = declaration.as_object_mut()
        {
            object.insert(
                "description".to_string(),
                serde_json::Value::String(description.to_string()),
            );
        }
        runtime_gemini_apply_gemini3_parameter_descriptions(&name, declaration);
    }
}

fn runtime_gemini_model_uses_gemini3_toolset(model: &str) -> bool {
    let model = model.to_ascii_lowercase();
    model.contains("gemini-3") || model == "auto" || model.contains("auto-gemini-3")
}

fn runtime_gemini_gemini3_tool_description(name: &str) -> Option<&'static str> {
    let aliases = runtime_gemini_tool_aliases(name);
    if aliases.contains("read_file") {
        return Some(
            "Read a file with targeted, surgical ranges. Prefer start_line and end_line when only part of the file is needed.",
        );
    }
    if aliases.contains("read_many_files") {
        return Some(
            "Read multiple files by paths or globs; honor git, Gemini, custom ignore rules, and default heavy-directory excludes.",
        );
    }
    if aliases.contains("grep") || aliases.contains("rg") || aliases.contains("rip_grep") {
        return Some(
            "Search text fast with ripgrep-style behavior. Use this before broad reads when locating symbols or literals.",
        );
    }
    if aliases.contains("exec_command") || aliases.contains("run_shell_command") {
        return Some(
            "Run a shell command. Prefer non-interactive commands and continue background sessions until the needed output is available.",
        );
    }
    if aliases.contains("write_file") {
        return Some(
            "Write complete file content. Do not use placeholders; preserve unrelated user changes.",
        );
    }
    if aliases.contains("apply_patch") || aliases.contains("replace") || aliases.contains("edit") {
        return Some(
            "Apply targeted file edits with exact context. Keep edits narrow and use this instead of describing changes. For Add File patches, every content line must start with '+', for example: *** Begin Patch\\n*** Add File: note.txt\\n+hello\\n*** End Patch.",
        );
    }
    None
}

fn runtime_gemini_apply_gemini3_parameter_descriptions(
    name: &str,
    declaration: &mut serde_json::Value,
) {
    let aliases = runtime_gemini_tool_aliases(name);
    if aliases.contains("read_file") {
        runtime_gemini_set_parameter_description(
            declaration,
            "start_line",
            "1-based first line to read when a targeted range is enough.",
        );
        runtime_gemini_set_parameter_description(
            declaration,
            "end_line",
            "1-based last line to read, inclusive.",
        );
    }
    if aliases.contains("grep") || aliases.contains("rg") || aliases.contains("rip_grep") {
        runtime_gemini_set_parameter_description(
            declaration,
            "pattern",
            "Literal or regex pattern to search for.",
        );
    }
    if aliases.contains("apply_patch") || aliases.contains("replace") || aliases.contains("edit") {
        declaration["parameters"]["properties"]["input"]["description"] =
            serde_json::Value::String(
                "Full apply_patch grammar string. For Add File, prefix every new file content line with '+'."
                    .to_string(),
            );
    }
}

fn runtime_gemini_set_parameter_description(
    declaration: &mut serde_json::Value,
    parameter: &str,
    description: &str,
) {
    let Some(properties) = declaration
        .get_mut("parameters")
        .and_then(|parameters| parameters.get_mut("properties"))
        .and_then(serde_json::Value::as_object_mut)
    else {
        return;
    };
    let Some(property) = properties
        .get_mut(parameter)
        .and_then(serde_json::Value::as_object_mut)
    else {
        return;
    };
    property
        .entry("description".to_string())
        .or_insert_with(|| serde_json::Value::String(description.to_string()));
}

fn runtime_gemini_function_declaration_from_chat_tool(
    tool: &serde_json::Value,
) -> Option<serde_json::Value> {
    let function = tool.get("function")?;
    let name = function.get("name").and_then(serde_json::Value::as_str)?;
    let default_parameters = serde_json::json!({"type": "object"});
    let parameters = function.get("parameters").unwrap_or(&default_parameters);
    let mut declaration = serde_json::json!({
        "name": name,
        "parameters": runtime_gemini_sanitize_function_schema(parameters),
    });
    if let Some(description) = function
        .get("description")
        .and_then(serde_json::Value::as_str)
    {
        declaration["description"] = serde_json::Value::String(description.to_string());
    }
    Some(declaration)
}

pub(super) fn runtime_gemini_tool_config_from_chat(
    chat: &serde_json::Value,
) -> Option<serde_json::Value> {
    let tool_choice = chat.get("tool_choice")?;
    if tool_choice.as_str() == Some("auto") {
        return None;
    }
    if tool_choice.as_str() == Some("none") {
        return Some(serde_json::json!({
            "functionCallingConfig": {
                "mode": "NONE",
            }
        }));
    }
    if tool_choice.as_str() == Some("required") {
        return Some(serde_json::json!({
            "functionCallingConfig": {
                "mode": "ANY",
            }
        }));
    }
    let name = tool_choice
        .get("function")
        .and_then(|function| function.get("name"))
        .and_then(serde_json::Value::as_str)
        .or_else(|| tool_choice.get("name").and_then(serde_json::Value::as_str))?;
    Some(serde_json::json!({
        "functionCallingConfig": {
            "mode": "ANY",
            "allowedFunctionNames": [name],
        }
    }))
}
