//! Chat-compatible token usage mapping helpers.

pub fn provider_core_chat_compatible_responses_usage(
    usage: &serde_json::Value,
    provider_metadata_key: &str,
) -> Option<serde_json::Value> {
    let input_tokens = usage
        .get("prompt_tokens")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let output_tokens = usage
        .get("completion_tokens")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let total_tokens = usage
        .get("total_tokens")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or_else(|| input_tokens.saturating_add(output_tokens));
    let cache_hit_tokens = usage
        .get("prompt_cache_hit_tokens")
        .and_then(serde_json::Value::as_u64);
    let cache_miss_tokens = usage
        .get("prompt_cache_miss_tokens")
        .and_then(serde_json::Value::as_u64);
    let reasoning_tokens = usage
        .get("completion_tokens_details")
        .and_then(|details| details.get("reasoning_tokens"))
        .and_then(serde_json::Value::as_u64);
    let mut response_usage = serde_json::json!({
        "input_tokens": input_tokens,
        "output_tokens": output_tokens,
        "total_tokens": total_tokens,
    });
    if let Some(cache_hit_tokens) = cache_hit_tokens {
        response_usage["input_tokens_details"] = serde_json::json!({
            "cached_tokens": cache_hit_tokens,
        });
    }
    if let Some(reasoning_tokens) = reasoning_tokens {
        response_usage["output_tokens_details"] = serde_json::json!({
            "reasoning_tokens": reasoning_tokens,
        });
    }
    if cache_hit_tokens.is_some() || cache_miss_tokens.is_some() {
        response_usage["metadata"] = serde_json::json!({
            provider_metadata_key: {
                "prompt_cache_hit_tokens": cache_hit_tokens.unwrap_or(0),
                "prompt_cache_miss_tokens": cache_miss_tokens.unwrap_or(0),
            }
        });
    }
    Some(response_usage)
}
