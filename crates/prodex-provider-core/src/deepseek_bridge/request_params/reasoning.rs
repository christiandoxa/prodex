//! DeepSeek reasoning-mode request parameter mapping.

pub fn deepseek_provider_core_apply_reasoning_from_responses_request(
    value: &serde_json::Value,
    request: &mut serde_json::Map<String, serde_json::Value>,
    provider_label: &str,
    gemini_compat: bool,
) -> Result<(), String> {
    let effort =
        deepseek_provider_core_reasoning_effort_from_responses_request(value, provider_label)?;
    if gemini_compat {
        if let Some(effort) = effort {
            let Some(effort) = deepseek_provider_core_gemini_openai_reasoning_effort(effort) else {
                return Err(format!(
                    "{provider_label} reasoning effort is not supported"
                ));
            };
            request.insert(
                "reasoning_effort".to_string(),
                serde_json::Value::String(effort.to_string()),
            );
        }
        return Ok(());
    }
    match effort.and_then(deepseek_provider_core_reasoning_effort) {
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
    if effort.is_some()
        && effort
            .and_then(deepseek_provider_core_reasoning_effort)
            .is_none()
    {
        return Err(format!(
            "{provider_label} reasoning effort is not supported"
        ));
    }
    Ok(())
}

pub fn deepseek_provider_core_thinking_enabled(value: &serde_json::Value) -> bool {
    deepseek_provider_core_reasoning_effort_from_responses_request(value, "DeepSeek")
        .ok()
        .flatten()
        .and_then(deepseek_provider_core_reasoning_effort)
        .is_some_and(|effort| effort.is_some())
}

pub fn deepseek_provider_core_validate_reasoning_shape(
    value: &serde_json::Value,
    provider_label: &str,
) -> Result<(), String> {
    deepseek_provider_core_reasoning_effort_from_responses_request(value, provider_label)
        .map(|_| ())
}

fn deepseek_provider_core_reasoning_effort_from_responses_request<'a>(
    value: &'a serde_json::Value,
    provider_label: &str,
) -> Result<Option<&'a str>, String> {
    if let Some(reasoning) = value.get("reasoning") {
        let Some(reasoning) = reasoning.as_object() else {
            return Err(format!("{provider_label} reasoning must be an object"));
        };
        for key in reasoning.keys() {
            if key != "effort" {
                return Err(format!(
                    "{provider_label} reasoning.{key} is not supported by this Responses adapter"
                ));
            }
        }
        if let Some(effort) = reasoning.get("effort") {
            return effort
                .as_str()
                .map(Some)
                .ok_or_else(|| format!("{provider_label} reasoning.effort must be a string"));
        }
    }
    if let Some(effort) = value.get("reasoning_effort") {
        return effort
            .as_str()
            .map(Some)
            .ok_or_else(|| format!("{provider_label} reasoning_effort must be a string"));
    }
    Ok(None)
}

fn deepseek_provider_core_gemini_openai_reasoning_effort(effort: &str) -> Option<&'static str> {
    match effort.trim().to_ascii_lowercase().as_str() {
        "xhigh" | "max" | "high" => Some("high"),
        "medium" => Some("medium"),
        "low" => Some("low"),
        "minimal" => Some("minimal"),
        "none" => Some("none"),
        _ => None,
    }
}

fn deepseek_provider_core_reasoning_effort(effort: &str) -> Option<Option<&'static str>> {
    match effort.trim().to_ascii_lowercase().as_str() {
        "xhigh" | "max" => Some(Some("max")),
        "high" | "medium" | "low" => Some(Some("high")),
        "minimal" | "none" => Some(None),
        _ => None,
    }
}
