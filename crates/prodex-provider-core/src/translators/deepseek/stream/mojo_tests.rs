use super::super::response::deepseek_stream_event_from_chat_value_rust;
use super::deepseek_transform_stream_event;
use crate::translator::{ProviderTransformInput, ProviderTransformLoss};
use crate::{ProviderEndpoint, ProviderId};
use serde_json::{Value, json};

fn assert_stream_parity(value: Value) {
    let data = serde_json::to_string(&value).expect("event JSON");
    let mut document = crate::mojo_json::Document::default();
    document.openai_chat_context(&value, None);
    let raw = std::str::from_utf8(&document.raw).expect("Serde emits UTF-8 JSON");
    let expected = deepseek_stream_event_from_chat_value_rust(&value);
    assert_eq!(
        prodex_mojo_core::json::transform_deepseek_chat_stream_event(&document.nodes, raw)
            .expect("Mojo DeepSeek stream transform"),
        expected,
        "kernel event={data}"
    );

    let event = format!("data: {data}\n\n");
    let actual = deepseek_transform_stream_event(
        ProviderId::DeepSeek,
        ProviderTransformInput::new(ProviderEndpoint::Responses, event.as_bytes()),
    );
    match expected {
        Some(expected) => {
            assert!(matches!(actual.loss, ProviderTransformLoss::Lossless));
            assert_eq!(
                actual.body.as_deref(),
                Some(expected.as_slice()),
                "event={data}"
            );
        }
        None => assert_eq!(
            actual.loss,
            ProviderTransformLoss::UnsupportedUpstream {
                reason:
                    "DeepSeek SSE event does not contain a supported text or function-call delta"
                        .into(),
            }
        ),
    }
}

#[test]
fn deepseek_stream_event_matches_rust_oracle_for_edge_shapes() {
    for value in [
        json!({}),
        json!([]),
        json!({"choices": []}),
        json!({"choices": [null]}),
        json!({"choices": [{"delta": null}]}),
        json!({"choices": [{"delta": {}}]}),
        json!({"choices": [{"delta": {"content": ""}}]}),
        json!({"choices": [{"delta": {"content": "東京\n\"quoted\""}, "finish_reason": "stop"}]}),
        json!({"choices": [{"delta": {"tool_calls": [], "content": "text"}}]}),
        json!({"choices": [{"delta": {"tool_calls": [{
            "id": "call-1",
            "function": {"name": "shell", "arguments": "{\"cmd\":\"ls\"}"}
        }], "content": "lower priority"}}]}),
        json!({"choices": [{"delta": {"tool_calls": [{
            "id": false,
            "function": {"name": 7, "arguments": "raw"}
        }], "content": "lower priority"}}]}),
        json!({"choices": [{"delta": {"tool_calls": [{
            "function": {"arguments": false}
        }], "content": "no text fallback"}}]}),
        json!({"choices": [
            {"delta": {}, "finish_reason": null},
            {"delta": {"content": "ignored second choice"}}
        ]}),
    ] {
        assert_stream_parity(value);
    }
}

#[test]
fn deepseek_stream_event_matches_rust_oracle_for_five_thousand_generated_events() {
    for index in 0..5_000 {
        let text = format!("event-{index}-東京-\"quoted\"");
        let value = match index % 8 {
            0 => json!({"choices": [{"delta": {"content": text}}]}),
            1 => json!({"choices": [{"delta": {"tool_calls": [{
                "id": format!("call-{index}"),
                "function": {
                    "name": "shell",
                    "arguments": format!(r#"{{"cmd":"run {index}"}}"#)
                }
            }]}}]}),
            2 => json!({"choices": [{"delta": {
                "content": text,
                "tool_calls": [{"function": {"arguments": {"index": index}}}]
            }}]}),
            3 => json!({"choices": [{"delta": {"content": ""}, "finish_reason": "stop"}]}),
            4 => json!({"choices": [{"delta": {
                "content": text,
                "tool_calls": [{"function": {"arguments": false}}]
            }}]}),
            5 => json!({"choices": [{"delta": {"tool_calls": [{
                "id": false,
                "function": {"arguments": ""}
            }]}}]}),
            6 => json!({"choices": [{"delta": {}}]}),
            _ => json!({"choices": [
                {"delta": {}, "finish_reason": null},
                {"delta": {"content": text}}
            ]}),
        };
        assert_stream_parity(value);
    }
}

#[test]
fn deepseek_done_and_invalid_json_keep_transport_boundary_behavior() {
    let done = deepseek_transform_stream_event(
        ProviderId::DeepSeek,
        ProviderTransformInput::new(ProviderEndpoint::Responses, b"data: [DONE]\n\n"),
    );
    assert!(matches!(done.loss, ProviderTransformLoss::Lossless));
    assert_eq!(
        done.body.as_deref(),
        Some(b"event: response.completed\ndata: {}\n\n".as_slice())
    );

    let invalid = deepseek_transform_stream_event(
        ProviderId::DeepSeek,
        ProviderTransformInput::new(ProviderEndpoint::Responses, b"data: {bad}\n\n"),
    );
    assert!(matches!(
        invalid.loss,
        ProviderTransformLoss::Rejected { .. }
    ));
}
