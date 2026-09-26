use super::{
    deepseek_provider_core_reject_beta_completion_fields,
    deepseek_provider_core_reject_unsupported_request_fields,
};
use serde_json::{Value, json};

#[test]
fn feature_off_rejections_use_mojo_precedence_and_type_errors() {
    let request_cases: [(Value, &str); 9] = [
        (
            json!({"frequency_penalty": null, "presence_penalty": 1, "n": 2}),
            "DeepSeek frequency_penalty is deprecated and is not forwarded by Prodex",
        ),
        (
            json!({"presence_penalty": 1, "n": 2}),
            "DeepSeek presence_penalty is deprecated and is not forwarded by Prodex",
        ),
        (
            json!({"seed": 1, "n": 2}),
            "DeepSeek n is not supported by this Responses adapter",
        ),
        (
            json!({"include": null, "store": "false"}),
            "DeepSeek include must be an array",
        ),
        (
            json!({"background": null}),
            "DeepSeek background must be a boolean",
        ),
        (
            json!({"truncation": 7}),
            "DeepSeek truncation must be a string",
        ),
        (json!({"text": []}), "DeepSeek text must be an object"),
        (
            json!({"parallel_tool_calls": "false"}),
            "DeepSeek parallel_tool_calls must be a boolean",
        ),
        (
            json!({"stream": true, "stream_options": {"include_usage": 0}}),
            "DeepSeek stream_options.include_usage must be a boolean",
        ),
    ];

    for (request, expected) in request_cases {
        assert_eq!(
            deepseek_provider_core_reject_unsupported_request_fields(&request, "DeepSeek")
                .unwrap_err(),
            expected,
        );
    }

    for (request, expected) in [
        (
            json!({"prefix": null, "suffix": "x", "prompt": "y"}),
            "Kiro chat prefix completion requires the beta chat endpoint, which is outside this Responses adapter",
        ),
        (
            json!({"suffix": null, "prompt": "y"}),
            "Kiro FIM suffix completion requires the beta /completions endpoint, which is outside this Responses adapter",
        ),
        (
            json!({"prompt": 1}),
            "Kiro prompt completions require the beta /completions endpoint, which is outside this Responses adapter",
        ),
    ] {
        assert_eq!(
            deepseek_provider_core_reject_beta_completion_fields(&request, "Kiro").unwrap_err(),
            expected,
        );
    }

    for request in [
        json!(null),
        json!(true),
        json!("request"),
        json!([]),
        json!(1),
    ] {
        assert_eq!(
            deepseek_provider_core_reject_unsupported_request_fields(&request, "DeepSeek"),
            Ok(()),
        );
        assert_eq!(
            deepseek_provider_core_reject_beta_completion_fields(&request, "DeepSeek"),
            Ok(()),
        );
    }

    let oversized =
        json!({"padding": "x".repeat(prodex_mojo_core::rich::DEEPSEEK_KERNEL_MAX_BYTES)});
    for reject in [
        deepseek_provider_core_reject_unsupported_request_fields,
        deepseek_provider_core_reject_beta_completion_fields,
    ] {
        assert!(
            reject(&oversized, "DeepSeek")
                .unwrap_err()
                .starts_with("DeepSeek request policy input exceeds 4194304 bytes")
        );
    }
}
