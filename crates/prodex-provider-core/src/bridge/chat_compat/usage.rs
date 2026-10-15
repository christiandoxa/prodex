//! Chat-compatible usage projection through compiled Mojo.
pub fn provider_core_chat_compatible_responses_usage(
    usage: &serde_json::Value,
    provider_metadata_key: &str,
) -> Option<serde_json::Value> {
    let key = serde_json::Value::String(provider_metadata_key.to_owned());
    let mut document = crate::mojo_json::Document::default();
    document.array([usage, &key]);
    let raw = std::str::from_utf8(&document.raw).expect("Serde emits UTF-8 JSON");
    let output = prodex_mojo_core::json::transform_chat_bridge_usage(&document.nodes, raw)
        .expect("compiled Mojo chat bridge usage mapping failed");
    Some(serde_json::from_slice(&output).expect("Mojo chat usage must be valid JSON"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn mojo_chat_usage_preserves_saturation_zero_and_invalid_numeric_rules() {
        let usage = json!({"prompt_tokens":u64::MAX,"completion_tokens":2,
            "prompt_cache_hit_tokens":0,"prompt_cache_miss_tokens":false,
            "completion_tokens_details":{"reasoning_tokens":0}});
        assert_eq!(
            provider_core_chat_compatible_responses_usage(&usage, "東京").unwrap(),
            json!({
                "input_tokens":u64::MAX,"output_tokens":2,"total_tokens":u64::MAX,
                "input_tokens_details":{"cached_tokens":0},"output_tokens_details":{"reasoning_tokens":0},
                "metadata":{"東京":{"prompt_cache_hit_tokens":0,"prompt_cache_miss_tokens":0}},
            })
        );
        assert_eq!(
            provider_core_chat_compatible_responses_usage(&json!(false), "unused").unwrap(),
            json!({"input_tokens":0,"output_tokens":0,"total_tokens":0})
        );
        assert_eq!(
            provider_core_chat_compatible_responses_usage(
                &json!({"prompt_tokens":8,"completion_tokens":-2,"total_tokens":0}),
                "unused"
            )
            .unwrap(),
            json!({"input_tokens":8,"output_tokens":0,"total_tokens":0})
        );
    }
}
