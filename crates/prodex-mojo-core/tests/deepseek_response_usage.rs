#![cfg(feature = "mojo-rich")]

use prodex_mojo_core::rich::{DeepSeekKernelInput, DeepSeekKernelOperation, deepseek_kernel};

fn map_usage(raw: &str, provider: &str) -> Vec<u8> {
    let mut input = DeepSeekKernelInput::new(DeepSeekKernelOperation::ResponseUsage);
    input.role = Some(provider);
    input.usage = Some(raw);
    deepseek_kernel(input).expect("DeepSeek usage kernel")
}

#[test]
fn response_usage_maps_cache_and_reasoning_details_in_mojo() {
    assert_eq!(
        map_usage(
            r#"{
                "prompt_tokens": 11,
                "completion_tokens": 7,
                "total_tokens": 18,
                "prompt_cache_hit_tokens": 5,
                "prompt_cache_miss_tokens": 6,
                "completion_tokens_details": {"reasoning_tokens": 2}
            }"#,
            "deepseek",
        ),
        br#"{"input_tokens":11,"output_tokens":7,"total_tokens":18,"input_tokens_details":{"cached_tokens":5},"output_tokens_details":{"reasoning_tokens":2},"metadata":{"deepseek":{"prompt_cache_hit_tokens":5,"prompt_cache_miss_tokens":6}}}"#.to_vec()
    );
}

#[test]
fn response_usage_defaults_invalid_fields_and_saturates_missing_total() {
    assert_eq!(
        map_usage(
            r#"{
                "prompt_tokens": 18446744073709551615,
                "completion_tokens": 1,
                "total_tokens": 1.5,
                "prompt_cache_hit_tokens": "bad",
                "completion_tokens_details": {"reasoning_tokens": -1}
            }"#,
            "deepseek-test",
        ),
        br#"{"input_tokens":18446744073709551615,"output_tokens":1,"total_tokens":18446744073709551615}"#.to_vec()
    );
}
