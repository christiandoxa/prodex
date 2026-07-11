//! Gemini thinking-config mapping from OpenAI-compatible reasoning fields.

use serde_json::{Value, json};

pub(in crate::translators::gemini) fn gemini_thinking_config_from_request(
    obj: &serde_json::Map<String, Value>,
    model: &str,
) -> Option<Value> {
    gemini_thinking_config(obj, model, None)
}

pub(super) fn gemini_thinking_config_with_budget_from_request(
    obj: &serde_json::Map<String, Value>,
    model: &str,
    thinking_budget_tokens: Option<u64>,
) -> Option<Value> {
    gemini_thinking_config(obj, model, thinking_budget_tokens)
}

fn gemini_thinking_config(
    obj: &serde_json::Map<String, Value>,
    model: &str,
    thinking_budget_tokens: Option<u64>,
) -> Option<Value> {
    let effort = obj
        .get("reasoning")
        .and_then(|reasoning| reasoning.get("effort"))
        .and_then(Value::as_str)
        .unwrap_or("high")
        .to_ascii_lowercase();
    if effort == "none" || effort == "minimal" {
        return Some(json!({
            "includeThoughts": false,
            "thinkingBudget": 0,
        }));
    }
    if gemini_provider_core_model_uses_thinking_level(model) {
        let level = match effort.as_str() {
            "low" => "LOW",
            "medium" => "MEDIUM",
            _ => "HIGH",
        };
        return Some(json!({
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
    Some(json!({
        "includeThoughts": true,
        "thinkingBudget": budget,
    }))
}

pub fn gemini_provider_core_model_uses_thinking_level(model: &str) -> bool {
    let model = model.to_ascii_lowercase();
    model.contains("gemini-3") || model.contains("gemma-3") || model.contains("gemma-4")
}
