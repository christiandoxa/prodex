use super::provider_bridge::RuntimeProviderBridgeKind;

pub(super) fn runtime_deepseek_apply_reasoning_from_responses_request(
    value: &serde_json::Value,
    request: &mut serde_json::Map<String, serde_json::Value>,
    provider_kind: RuntimeProviderBridgeKind,
) {
    let effort = runtime_deepseek_reasoning_effort_from_responses_request(value);
    if provider_kind == RuntimeProviderBridgeKind::Gemini {
        if let Some(effort) = effort.and_then(runtime_gemini_openai_reasoning_effort) {
            request.insert(
                "reasoning_effort".to_string(),
                serde_json::Value::String(effort.to_string()),
            );
        }
        return;
    }
    match effort.and_then(runtime_deepseek_reasoning_effort) {
        Some(Some(effort)) => {
            request.insert(
                "thinking".to_string(),
                serde_json::json!({"type": "enabled"}),
            );
            request.insert(
                "reasoning_effort".to_string(),
                serde_json::Value::String(effort.to_string()),
            );
        }
        Some(None) => {
            request.insert(
                "thinking".to_string(),
                serde_json::json!({"type": "disabled"}),
            );
        }
        None => {}
    }
}

fn runtime_gemini_openai_reasoning_effort(effort: &str) -> Option<&'static str> {
    match effort.trim().to_ascii_lowercase().as_str() {
        "xhigh" | "max" | "high" => Some("high"),
        "medium" => Some("medium"),
        "low" => Some("low"),
        "minimal" => Some("minimal"),
        "none" => Some("none"),
        _ => None,
    }
}

pub(super) fn runtime_deepseek_thinking_enabled(value: &serde_json::Value) -> bool {
    runtime_deepseek_reasoning_effort_from_responses_request(value)
        .and_then(runtime_deepseek_reasoning_effort)
        .is_some_and(|effort| effort.is_some())
}

fn runtime_deepseek_reasoning_effort_from_responses_request(
    value: &serde_json::Value,
) -> Option<&str> {
    value
        .get("reasoning")
        .and_then(|reasoning| reasoning.get("effort"))
        .and_then(serde_json::Value::as_str)
        .or_else(|| {
            value
                .get("reasoning_effort")
                .and_then(serde_json::Value::as_str)
        })
}

fn runtime_deepseek_reasoning_effort(effort: &str) -> Option<Option<&'static str>> {
    match effort.trim().to_ascii_lowercase().as_str() {
        "xhigh" | "max" => Some(Some("max")),
        "high" | "medium" | "low" => Some(Some("high")),
        "minimal" | "none" => Some(None),
        _ => None,
    }
}
