//! DeepSeek response-format metadata and degraded JSON-mode notes.

pub fn deepseek_provider_core_response_format_from_responses_request(
    value: &serde_json::Value,
    provider_label: &str,
) -> Result<Option<serde_json::Value>, String> {
    let response_format = value
        .get("response_format")
        .or_else(|| value.get("text").and_then(|text| text.get("format")));
    let Some(response_format) = response_format else {
        return Ok(None);
    };
    let format_type = response_format
        .get("type")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    match format_type {
        "json_object" | "json_schema" | "json" | "structured_output" => {
            Ok(Some(serde_json::json!({"type": "json_object"})))
        }
        "text" => Ok(None),
        "" => Err(format!(
            "{provider_label} response_format must include a type"
        )),
        other => Err(format!(
            "{provider_label} response_format type `{other}` is not supported"
        )),
    }
}

pub fn deepseek_provider_core_response_metadata_from_responses_request(
    value: &serde_json::Value,
    provider_label: &str,
    provider_key: &str,
) -> Result<Option<serde_json::Value>, String> {
    let mut metadata = match value.get("metadata") {
        Some(metadata) => metadata
            .as_object()
            .cloned()
            .ok_or_else(|| format!("{provider_label} request metadata must be an object"))?,
        None => serde_json::Map::new(),
    };
    if metadata
        .get(provider_key)
        .is_some_and(|provider_metadata| !provider_metadata.is_object())
    {
        return Err(format!(
            "{provider_label} request metadata.{provider_key} must be an object"
        ));
    }
    if let Some(client_metadata) = value.get("client_metadata") {
        let client_metadata = client_metadata
            .as_object()
            .cloned()
            .ok_or_else(|| format!("{provider_label} client_metadata must be an object"))?;
        metadata.insert(
            "client_metadata".to_string(),
            serde_json::Value::Object(client_metadata),
        );
    }
    if let Some(prompt_cache_key) = value.get("prompt_cache_key") {
        let prompt_cache_key = prompt_cache_key
            .as_str()
            .ok_or_else(|| format!("{provider_label} prompt_cache_key must be a string"))?;
        if !prompt_cache_key.trim().is_empty() {
            metadata.insert(
                "prompt_cache_key".to_string(),
                serde_json::Value::String(prompt_cache_key.to_string()),
            );
        }
    }
    if let Some(prompt_cache_retention) = value.get("prompt_cache_retention") {
        if !prompt_cache_retention.is_string() {
            return Err(format!(
                "{provider_label} prompt_cache_retention must be a string"
            ));
        }
        metadata.insert(
            "prompt_cache_retention".to_string(),
            prompt_cache_retention.clone(),
        );
    }
    let response_format = value
        .get("response_format")
        .or_else(|| value.get("text").and_then(|text| text.get("format")));
    if let Some(response_format) = response_format {
        let format_type = response_format
            .get("type")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        if matches!(format_type, "json_schema" | "structured_output") {
            let provider_metadata = metadata
                .entry(provider_key.to_string())
                .or_insert_with(|| serde_json::json!({}))
                .as_object_mut()
                .ok_or_else(|| {
                    format!("{provider_label} request metadata.{provider_key} must be an object")
                })?;
            provider_metadata.insert(
                "degraded_response_format".to_string(),
                serde_json::json!({
                    "from": format_type,
                    "to": "json_object",
                    "reason": format!(
                        "{provider_label} response_format supports json_object but not native JSON Schema enforcement"
                    )
                }),
            );
        }
    }
    Ok((!metadata.is_empty()).then_some(serde_json::Value::Object(metadata)))
}

pub fn deepseek_provider_core_note_thinking_tool_choice_omission(
    value: &serde_json::Value,
    thinking_enabled: bool,
    provider_label: &str,
    provider_key: &str,
    response_metadata: &mut Option<serde_json::Value>,
) {
    if !thinking_enabled {
        return;
    }
    let Some(tool_choice) = value.get("tool_choice") else {
        return;
    };
    let metadata = response_metadata
        .get_or_insert_with(|| serde_json::json!({}))
        .as_object_mut();
    let Some(metadata) = metadata else {
        return;
    };
    let provider_metadata = metadata
        .entry(provider_key.to_string())
        .or_insert_with(|| serde_json::json!({}))
        .as_object_mut();
    let Some(provider_metadata) = provider_metadata else {
        return;
    };
    provider_metadata.insert(
        "omitted_tool_choice".to_string(),
        serde_json::json!({
            "from": tool_choice,
            "reason": format!(
                "{provider_label} thinking mode currently rejects explicit tool_choice on the OpenAI Chat route, so Prodex omits it while preserving translated function tools"
            )
        }),
    );
}

pub fn deepseek_provider_core_ensure_json_prompt_instruction(
    messages: &mut Vec<serde_json::Value>,
) {
    if messages
        .iter()
        .any(deepseek_provider_core_message_has_json_guidance)
    {
        return;
    }
    messages.insert(
        0,
        serde_json::json!({
            "role": "system",
            "content": "Respond with valid JSON only.",
        }),
    );
}

fn deepseek_provider_core_message_has_json_guidance(message: &serde_json::Value) -> bool {
    if !matches!(
        message.get("role").and_then(serde_json::Value::as_str),
        Some("system" | "user")
    ) {
        return false;
    }
    message
        .get("content")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|content| content.to_ascii_lowercase().contains("json"))
}
