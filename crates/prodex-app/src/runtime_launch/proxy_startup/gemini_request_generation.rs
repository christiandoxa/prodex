pub(super) fn runtime_gemini_generation_config(
    original: &serde_json::Value,
    chat: &serde_json::Value,
    model: &str,
    thinking_budget_tokens: Option<u64>,
) -> serde_json::Value {
    let mut config = serde_json::Map::new();
    for (from, to) in [
        ("temperature", "temperature"),
        ("top_p", "topP"),
        ("max_tokens", "maxOutputTokens"),
    ] {
        if let Some(value) = chat.get(from) {
            config.insert(to.to_string(), value.clone());
        }
    }
    for (from, to) in [
        ("top_k", "topK"),
        ("topK", "topK"),
        ("candidate_count", "candidateCount"),
        ("candidateCount", "candidateCount"),
        ("seed", "seed"),
        ("presence_penalty", "presencePenalty"),
        ("presencePenalty", "presencePenalty"),
        ("frequency_penalty", "frequencyPenalty"),
        ("frequencyPenalty", "frequencyPenalty"),
        ("response_mime_type", "responseMimeType"),
        ("responseMimeType", "responseMimeType"),
        ("response_schema", "responseSchema"),
        ("responseSchema", "responseSchema"),
        ("response_json_schema", "responseJsonSchema"),
        ("responseJsonSchema", "responseJsonSchema"),
        ("response_modalities", "responseModalities"),
        ("responseModalities", "responseModalities"),
        ("media_resolution", "mediaResolution"),
        ("mediaResolution", "mediaResolution"),
        ("audio_timestamp", "audioTimestamp"),
        ("audioTimestamp", "audioTimestamp"),
        ("speech_config", "speechConfig"),
        ("speechConfig", "speechConfig"),
    ] {
        if let Some(value) = original.get(from).filter(|value| !value.is_null()) {
            config.insert(to.to_string(), value.clone());
        }
    }
    if let Some(stop) = original
        .get("stop")
        .or_else(|| original.get("stop_sequences"))
        .or_else(|| original.get("stopSequences"))
        .filter(|value| !value.is_null())
    {
        config.insert("stopSequences".to_string(), stop.clone());
    }
    runtime_gemini_apply_text_format(original, &mut config);
    if let Some(thinking_config) =
        runtime_gemini_thinking_config(original, model, thinking_budget_tokens)
    {
        config.insert("thinkingConfig".to_string(), thinking_config);
    }
    serde_json::Value::Object(config)
}

fn runtime_gemini_apply_text_format(
    original: &serde_json::Value,
    config: &mut serde_json::Map<String, serde_json::Value>,
) {
    let Some(text) = original.get("text").and_then(serde_json::Value::as_object) else {
        return;
    };
    let Some(format) = text.get("format").and_then(serde_json::Value::as_object) else {
        return;
    };
    let format_type = format
        .get("type")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    match format_type {
        "json_object" => {
            config.insert(
                "responseMimeType".to_string(),
                serde_json::Value::String("application/json".to_string()),
            );
        }
        "json_schema" => {
            config.insert(
                "responseMimeType".to_string(),
                serde_json::Value::String("application/json".to_string()),
            );
            if let Some(schema) = format.get("schema").or_else(|| format.get("json_schema")) {
                config.insert("responseJsonSchema".to_string(), schema.clone());
            }
        }
        _ => {}
    }
}

fn runtime_gemini_thinking_config(
    original: &serde_json::Value,
    model: &str,
    thinking_budget_tokens: Option<u64>,
) -> Option<serde_json::Value> {
    let effort = original
        .get("reasoning")
        .and_then(|reasoning| reasoning.get("effort"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("high")
        .to_ascii_lowercase();
    if effort == "none" || effort == "minimal" {
        return Some(serde_json::json!({
            "includeThoughts": false,
            "thinkingBudget": 0,
        }));
    }
    if runtime_gemini_model_uses_thinking_level(model) {
        let level = match effort.as_str() {
            "low" => "LOW",
            "medium" => "MEDIUM",
            _ => "HIGH",
        };
        return Some(serde_json::json!({
            "includeThoughts": true,
            "thinkingLevel": level,
        }));
    }
    let budget = match (thinking_budget_tokens, effort.as_str()) {
        (Some(budget), _) => budget,
        (None, "low") => 1024,
        (None, "medium" | "high") => 8192,
        (None, "xhigh") => 24576,
        (None, _) => 8192,
    };
    Some(serde_json::json!({
        "includeThoughts": true,
        "thinkingBudget": budget,
    }))
}

fn runtime_gemini_model_uses_thinking_level(model: &str) -> bool {
    model.contains("gemini-3") || model.contains("gemma-3") || model.contains("gemma-4")
}
