pub fn runtime_anthropic_tool_input_from_arguments(arguments: &str) -> serde_json::Value {
    serde_json::from_str::<serde_json::Value>(arguments)
        .ok()
        .filter(|value| value.is_object())
        .unwrap_or_else(|| serde_json::Value::Object(serde_json::Map::new()))
}

pub fn runtime_anthropic_reasoning_summary_text(item: &serde_json::Value) -> String {
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
