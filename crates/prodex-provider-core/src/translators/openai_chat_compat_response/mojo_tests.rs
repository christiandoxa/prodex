use super::{
    ProviderEndpoint, ProviderId, ProviderTransformInput, translate_chat_response_body_rust,
    translate_chat_response_to_responses_at,
};
use crate::ProviderTransformLoss;
use serde_json::{Value, json};

fn assert_response_parity(response: Value, now_secs: u64) {
    let input = ProviderTransformInput::new(
        ProviderEndpoint::Responses,
        serde_json::to_vec(&response).expect("response serializes"),
    );
    let actual = translate_chat_response_to_responses_at(ProviderId::Anthropic, input, now_secs);
    assert!(matches!(actual.loss, ProviderTransformLoss::Lossless));
    let actual: Value = serde_json::from_slice(&actual.body.expect("response body"))
        .expect("Mojo response is JSON");
    let expected: Value =
        serde_json::from_slice(&translate_chat_response_body_rust(&response, now_secs))
            .expect("Rust oracle response is JSON");
    assert_eq!(actual, expected, "response={response}");
}

#[test]
fn response_matches_rust_oracle_for_sparse_content_tool_and_usage_shapes() {
    for response in [
        json!({}),
        json!({"choices": []}),
        json!("not an object"),
        json!({"created": 42.0, "choices": [null]}),
        json!({
            "id": 9,
            "model": "compat-東京",
            "created": "bad",
            "choices": [{
                "message": {
                    "role": null,
                    "content": [
                        {"text": "alpha"},
                        {"text": false, "content": "beta"},
                        {"text": "", "content": "ignored"},
                        "ignored",
                        {"content": "γ\nδ"}
                    ],
                    "tool_calls": [
                        {"function": {"name": "functions.exec_command", "arguments": "{\"cmd\":\"ls\"}"}},
                        {"id": "tool", "function": {"name": "functions.sub.tool", "arguments": {"x": true}}},
                        {"function": {"name": ".leading", "arguments": [1, 2]}},
                        {"id": false, "function": {"name": "missing-args"}},
                        {"function": {"name": 5}}
                    ]
                }
            }, {"message": {"content": "ignored second choice"}}],
            "usage": {
                "prompt_tokens": 4,
                "input_tokens": 99,
                "completion_tokens": "invalid",
                "output_tokens": 8,
                "total_tokens": "invalid"
            }
        }),
        json!({
            "choices": [{
                "message": {
                    "content": {
                        "content": [{"text": "left"}, {"content": "right"}],
                        "output_text": "blocked by content"
                    }
                }
            }],
            "usage": {"input_tokens": 7, "output_tokens": 5, "total_tokens": 18}
        }),
        json!({
            "choices": [{"message": {"content": {"text": "", "content": "blocked", "output_text": "blocked"}}}],
            "usage": {"prompt_tokens": "invalid", "input_tokens": 4}
        }),
    ] {
        assert_response_parity(response, 1_700_000_123);
    }
}

#[test]
fn response_matches_rust_oracle_for_five_thousand_generated_responses() {
    for index in 0..5_000 {
        let text = format!("case-{index}-東京-\"\\\n");
        let content = match index % 6 {
            0 => json!(text),
            1 => json!([{"text": text}, {"content": "tail"}]),
            2 => json!({"text": text, "content": "lower priority"}),
            3 => json!({"content": {"text": text}}),
            4 => json!({"output_text": text}),
            _ => json!([]),
        };
        let tool_call = match index % 4 {
            0 => json!({"id": format!("call-{index}"), "function": {
                "name": "functions.exec_command",
                "arguments": format!(r#"{{"cmd":"cargo test {index}"}}"#)
            }}),
            1 => json!({"function": {"name": "tools.sub.tool", "arguments": {
                "unicode": "東京", "index": index
            }}}),
            2 => json!({"function": {"name": ".leading", "arguments": [index, true]}}),
            _ => json!({"function": {"name": "", "arguments": {}}}),
        };
        let mut response = json!({"choices": [{"message": {
            "content": content, "tool_calls": [tool_call]
        }}]});
        if index % 3 != 0 {
            response["id"] = json!(format!("resp-{index}"));
        }
        if index % 5 == 0 {
            response["model"] = json!("model-東京");
        }
        match index % 4 {
            0 => response["created"] = json!(index),
            1 => response["created"] = json!("invalid"),
            2 => response["created"] = json!(index as f64),
            _ => {}
        }
        if index % 4 != 0 {
            response["usage"] = match index % 3 {
                0 => json!({"prompt_tokens": index, "completion_tokens": 5}),
                1 => json!({"input_tokens": index, "output_tokens": 2, "total_tokens": index + 10}),
                _ => json!({"prompt_tokens": "bad", "input_tokens": index}),
            };
        }
        assert_response_parity(response, 1_700_000_000 + index as u64);
    }
}
