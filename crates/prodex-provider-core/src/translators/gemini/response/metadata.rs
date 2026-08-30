//! Gemini response metadata and usage normalization.

use serde_json::{Value, json};

pub(crate) fn gemini_responses_usage(usage: &Value) -> Option<Value> {
    let input_tokens = usage
        .get("promptTokenCount")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let output_tokens = usage
        .get("candidatesTokenCount")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let cached_tokens = usage
        .get("cachedContentTokenCount")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let reasoning_tokens = usage
        .get("thoughtsTokenCount")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let tool_tokens = usage
        .get("toolUsePromptTokenCount")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    #[cfg(feature = "mojo")]
    {
        let mut input = prodex_mojo_core::rich::GeminiResponseKernelInput::new(
            prodex_mojo_core::rich::GeminiResponseKernelOperation::ResponseUsage,
        );
        input.prompt_token_count = input_tokens;
        input.candidate_token_count = output_tokens;
        input.total_token_count_present = i64::from(usage.get("totalTokenCount").is_some());
        input.total_token_count = usage
            .get("totalTokenCount")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        input.cached_content_token_count = cached_tokens;
        input.thoughts_token_count = reasoning_tokens;
        input.tool_use_prompt_token_count = tool_tokens;
        return Some(super::super::stream::gemini_mojo_value(input));
    }
    #[cfg(not(feature = "mojo"))]
    {
        let total_tokens = usage
            .get("totalTokenCount")
            .and_then(Value::as_u64)
            .unwrap_or_else(|| input_tokens.saturating_add(output_tokens));
        Some(json!({
            "input_tokens": input_tokens,
            "input_tokens_details": {
                "cached_tokens": cached_tokens,
                "tool_tokens": tool_tokens,
            },
            "output_tokens": output_tokens,
            "output_tokens_details": {
                "reasoning_tokens": reasoning_tokens,
            },
            "total_tokens": total_tokens,
        }))
    }
}

pub(crate) fn gemini_response_metadata(value: &Value) -> Option<Value> {
    let mut gemini = serde_json::Map::new();
    for key in ["promptFeedback", "usageMetadata"] {
        if let Some(field) = value.get(key).filter(|field| !field.is_null()) {
            gemini.insert(key.to_string(), field.clone());
        }
    }
    if let Some(candidate) = value
        .get("candidates")
        .and_then(Value::as_array)
        .and_then(|candidates| candidates.first())
    {
        for key in [
            "finishReason",
            "finishMessage",
            "safetyRatings",
            "citationMetadata",
            "groundingMetadata",
            "urlContextMetadata",
            "avgLogprobs",
            "logprobsResult",
        ] {
            if let Some(field) = candidate.get(key).filter(|field| !field.is_null()) {
                gemini.insert(key.to_string(), field.clone());
            }
        }
    }
    if gemini.is_empty() {
        return None;
    }
    Some(json!({
        "gemini": Value::Object(gemini),
    }))
}
