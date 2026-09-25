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
    let total_tokens = usage.get("totalTokenCount").and_then(Value::as_u64);
    let mut input = prodex_mojo_core::rich::GeminiResponseKernelInput::new(
        prodex_mojo_core::rich::GeminiResponseKernelOperation::ResponseUsage,
    );
    input.prompt_token_count = input_tokens;
    input.candidate_token_count = output_tokens;
    input.total_token_count_present = i64::from(total_tokens.is_some());
    input.total_token_count = total_tokens.unwrap_or_default();
    input.cached_content_token_count = cached_tokens;
    input.thoughts_token_count = reasoning_tokens;
    input.tool_use_prompt_token_count = tool_tokens;
    Some(super::super::stream::gemini_mojo_value(input))
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn builds_usage_from_gemini_counts() {
        assert_eq!(
            gemini_responses_usage(&json!({
                "promptTokenCount": 11,
                "candidatesTokenCount": 7,
                "totalTokenCount": 23,
                "cachedContentTokenCount": 3,
                "thoughtsTokenCount": 5,
                "toolUsePromptTokenCount": 2,
            })),
            Some(json!({
                "input_tokens": 11,
                "input_tokens_details": {
                    "cached_tokens": 3,
                    "tool_tokens": 2,
                },
                "output_tokens": 7,
                "output_tokens_details": {
                    "reasoning_tokens": 5,
                },
                "total_tokens": 23,
            }))
        );
    }

    #[test]
    fn absent_or_invalid_total_uses_saturating_input_sum() {
        let counts = json!({
            "promptTokenCount": 11,
            "candidatesTokenCount": 7,
        });
        let expected_total = json!({
            "input_tokens": 11,
            "input_tokens_details": {"cached_tokens": 0, "tool_tokens": 0},
            "output_tokens": 7,
            "output_tokens_details": {"reasoning_tokens": 0},
            "total_tokens": 18,
        });
        assert_eq!(
            gemini_responses_usage(&counts),
            Some(expected_total.clone())
        );
        for total in [json!(null), json!(-1), json!("18"), json!(18.0)] {
            let mut usage = counts.clone();
            usage["totalTokenCount"] = total;
            assert_eq!(gemini_responses_usage(&usage), Some(expected_total.clone()));
        }

        assert_eq!(
            gemini_responses_usage(&json!({
                "promptTokenCount": u64::MAX,
                "candidatesTokenCount": 1,
            }))
            .unwrap()["total_tokens"],
            json!(u64::MAX)
        );
    }

    #[test]
    fn explicit_zero_total_and_invalid_counts_keep_their_values() {
        assert_eq!(
            gemini_responses_usage(&json!({
                "promptTokenCount": -1,
                "candidatesTokenCount": "7",
                "totalTokenCount": 0,
                "cachedContentTokenCount": 3.5,
                "thoughtsTokenCount": null,
                "toolUsePromptTokenCount": 2,
            })),
            Some(json!({
                "input_tokens": 0,
                "input_tokens_details": {"cached_tokens": 0, "tool_tokens": 2},
                "output_tokens": 0,
                "output_tokens_details": {"reasoning_tokens": 0},
                "total_tokens": 0,
            }))
        );
    }
}
