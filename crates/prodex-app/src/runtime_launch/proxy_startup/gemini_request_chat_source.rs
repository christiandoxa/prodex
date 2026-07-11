pub(super) fn runtime_gemini_chat_source_request(
    original: &serde_json::Value,
) -> serde_json::Value {
    let mut value = original.clone();
    let Some(object) = value.as_object_mut() else {
        return value;
    };
    for field in [
        "tools",
        "tool_choice",
        "web_search_options",
        "frequency_penalty",
        "presence_penalty",
        "n",
        "seed",
        "service_tier",
        "prediction",
        "logit_bias",
        "functions",
        "function_call",
    ] {
        object.remove(field);
    }
    if let Some(input) = object.get_mut("input") {
        runtime_gemini_sanitize_chat_source_input(input);
    }
    value
}

fn runtime_gemini_sanitize_chat_source_input(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Array(items) => {
            for item in items {
                runtime_gemini_sanitize_chat_source_item(item);
            }
        }
        serde_json::Value::Object(_) => runtime_gemini_sanitize_chat_source_item(value),
        _ => {}
    }
}

fn runtime_gemini_sanitize_chat_source_item(value: &mut serde_json::Value) {
    let Some(object) = value.as_object_mut() else {
        return;
    };
    if object.get("type").and_then(serde_json::Value::as_str) != Some("message") {
        return;
    }
    let Some(content) = object.get_mut("content") else {
        return;
    };
    let serde_json::Value::Array(parts) = content else {
        return;
    };
    parts.retain(runtime_gemini_chat_source_content_part_supported);
}

fn runtime_gemini_chat_source_content_part_supported(part: &serde_json::Value) -> bool {
    let Some(object) = part.as_object() else {
        return true;
    };
    object
        .get("text")
        .or_else(|| object.get("input_text"))
        .or_else(|| object.get("output_text"))
        .or_else(|| object.get("content"))
        .and_then(serde_json::Value::as_str)
        .is_some()
}
