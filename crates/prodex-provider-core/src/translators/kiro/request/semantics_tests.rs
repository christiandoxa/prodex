use super::{
    KiroProviderCoreRequestError, kiro_provider_core_chat_completions_request_body,
    kiro_provider_core_responses_request_body,
};
use serde_json::{Value, json};

#[test]
fn kiro_request_fixtures_capture_mojo_owned_semantics() {
    let chat = kiro_provider_core_chat_completions_request_body(
        r#"{"model":"auto","messages":[{"role":"user","content":"héllo 界"}]}"#.as_bytes(),
    )
    .unwrap();
    assert_eq!(
        serde_json::from_slice::<Value>(&chat).unwrap(),
        json!({
            "model": "auto",
            "input": [{
                "type": "message",
                "role": "user",
                "content": [{"type": "input_text", "text": "héllo 界"}]
            }]
        })
    );

    let messages = kiro_provider_core_responses_request_body(
        r#"{"model":"auto","system":"system 世界","messages":[{"role":"user","content":[{"type":"text","text":"café"}]}]}"#.as_bytes(),
        true,
    )
    .unwrap();
    assert_eq!(
        serde_json::from_slice::<Value>(&messages).unwrap(),
        json!({
            "model": "auto",
            "input": [
                {
                    "type": "message",
                    "role": "system",
                    "content": [{"type": "input_text", "text": "system 世界"}]
                },
                {
                    "type": "message",
                    "role": "user",
                    "content": [{"type": "input_text", "text": "café"}]
                }
            ]
        })
    );

    let ordered = kiro_provider_core_responses_request_body(
        br#"{"metadata":{"z":1,"a":2},"input":"hello","model":"auto"}"#,
        false,
    )
    .unwrap();
    assert_eq!(
        ordered,
        br#"{"model":"auto","input":"hello","metadata":{"a":2,"z":1}}"#
    );

    let invalid = kiro_provider_core_chat_completions_request_body(b"[]").unwrap_err();
    assert_eq!(
        invalid,
        KiroProviderCoreRequestError {
            message: "Kiro chat completions request body must be a JSON object".into(),
            code: "invalid_request_body".into(),
        }
    );

    let wrong_type = kiro_provider_core_responses_request_body(
        br#"{"model":"auto","input":"hello","logprobs":"yes"}"#,
        false,
    )
    .unwrap_err();
    assert_eq!(
        wrong_type,
        KiroProviderCoreRequestError {
            message: "Kiro logprobs must be a boolean".into(),
            code: "invalid_logprobs".into(),
        }
    );

    let precedence = kiro_provider_core_chat_completions_request_body(
        br#"{"messages":[],"response_format":{"type":"json_object"},"n":2,"stop":["end"]}"#,
    )
    .unwrap_err();
    assert_eq!(precedence.code, "unsupported_response_format");
    assert_eq!(
        precedence.message,
        "Kiro provider only supports chat response_format type 'text' right now"
    );

    let precedence = kiro_provider_core_responses_request_body(
        br#"{"model":"auto","input":"hello","temperature":0.5,"stop":["end"],"logprobs":"yes"}"#,
        false,
    )
    .unwrap_err();
    assert_eq!(precedence.code, "unsupported_generation_control");
    assert_eq!(
        precedence.message,
        "Kiro ACP does not expose the temperature control"
    );
}
