//! Gemini Live setup message builders.

pub fn gemini_provider_core_live_function_declaration(
    tool: &serde_json::Value,
) -> Option<serde_json::Value> {
    if tool.get("type").and_then(serde_json::Value::as_str) != Some("function") {
        return None;
    }
    let name = tool.get("name").and_then(serde_json::Value::as_str)?;
    let mut declaration = serde_json::json!({
        "name": name,
        "parameters": tool
            .get("parameters")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({"type": "object"})),
    });
    if let Some(description) = tool.get("description").filter(|value| !value.is_null()) {
        declaration["description"] = description.clone();
    }
    Some(declaration)
}

pub fn gemini_provider_core_live_setup_message(
    session: &serde_json::Map<String, serde_json::Value>,
    default_model: &str,
    configured_model: Option<&str>,
) -> serde_json::Value {
    let requested_model = session
        .get("model")
        .and_then(serde_json::Value::as_str)
        .filter(|model| model.starts_with("gemini-"));
    let model = configured_model
        .filter(|model| !model.trim().is_empty())
        .or(requested_model)
        .unwrap_or(default_model);
    let model = model.strip_prefix("models/").unwrap_or(model);
    let output_modalities = session
        .get("output_modalities")
        .and_then(serde_json::Value::as_array)
        .map(|modalities| {
            modalities
                .iter()
                .filter_map(serde_json::Value::as_str)
                .map(|modality| modality.to_ascii_uppercase())
                .collect::<Vec<_>>()
        })
        .filter(|modalities| !modalities.is_empty())
        .unwrap_or_else(|| vec!["AUDIO".to_string()]);
    let mut setup = serde_json::json!({
        "model": format!("models/{model}"),
        "generation_config": {
            "response_modalities": output_modalities,
        },
        "input_audio_transcription": {},
        "output_audio_transcription": {},
    });
    if let Some(instructions) = session
        .get("instructions")
        .and_then(serde_json::Value::as_str)
        .filter(|instructions| !instructions.trim().is_empty())
    {
        setup["system_instruction"] = serde_json::json!({
            "parts": [{"text": instructions}],
        });
    }
    if let Some(tools) = session
        .get("tools")
        .and_then(serde_json::Value::as_array)
        .map(|tools| {
            tools
                .iter()
                .filter_map(gemini_provider_core_live_function_declaration)
                .collect::<Vec<_>>()
        })
        .filter(|tools| !tools.is_empty())
    {
        setup["tools"] = serde_json::json!([{"function_declarations": tools}]);
    }
    serde_json::json!({"setup": setup})
}
