use super::*;

pub(crate) fn translate_runtime_anthropic_messages_request(
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
    translated_body.insert("stream".to_string(), serde_json::Value::Bool(true));
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
