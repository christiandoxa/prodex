use super::provider_bridge::RuntimeProviderBridgeKind;
use anyhow::Result;

pub(super) fn runtime_deepseek_apply_reasoning_from_responses_request(
    value: &serde_json::Value,
    request: &mut serde_json::Map<String, serde_json::Value>,
    provider_kind: RuntimeProviderBridgeKind,
) -> Result<()> {
    let effort = runtime_deepseek_reasoning_effort_from_responses_request(value)?;
    if provider_kind == RuntimeProviderBridgeKind::Gemini {
        if let Some(effort) = effort {
            let Some(effort) = runtime_gemini_openai_reasoning_effort(effort) else {
                anyhow::bail!("Gemini OpenAI reasoning effort is not supported");
            };
            request.insert(
                "reasoning_effort".to_string(),
                serde_json::Value::String(effort.to_string()),
            );
        }
        return Ok(());
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
    if effort.is_some() && effort.and_then(runtime_deepseek_reasoning_effort).is_none() {
        anyhow::bail!("DeepSeek reasoning effort is not supported");
    }
    Ok(())
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
        .ok()
        .flatten()
        .and_then(runtime_deepseek_reasoning_effort)
        .is_some_and(|effort| effort.is_some())
}

fn runtime_deepseek_reasoning_effort_from_responses_request(
    value: &serde_json::Value,
) -> Result<Option<&str>> {
    if let Some(reasoning) = value.get("reasoning") {
        let Some(reasoning) = reasoning.as_object() else {
            anyhow::bail!("DeepSeek reasoning must be an object");
        };
        for key in reasoning.keys() {
            if key != "effort" {
                anyhow::bail!(
                    "DeepSeek reasoning.{key} is not supported by this Responses adapter"
                );
            }
        }
        if let Some(effort) = reasoning.get("effort") {
            return effort
                .as_str()
                .map(Some)
                .ok_or_else(|| anyhow::anyhow!("DeepSeek reasoning.effort must be a string"));
        }
    }
    if let Some(effort) = value.get("reasoning_effort") {
        return effort
            .as_str()
            .map(Some)
            .ok_or_else(|| anyhow::anyhow!("DeepSeek reasoning_effort must be a string"));
    }
    Ok(None)
}

fn runtime_deepseek_reasoning_effort(effort: &str) -> Option<Option<&'static str>> {
    match effort.trim().to_ascii_lowercase().as_str() {
        "xhigh" | "max" => Some(Some("max")),
        "high" | "medium" | "low" => Some(Some("high")),
        "minimal" | "none" => Some(None),
        _ => None,
    }
}
