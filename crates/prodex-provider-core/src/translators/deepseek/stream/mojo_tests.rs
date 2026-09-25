use super::deepseek_transform_stream_event;
use crate::translator::{ProviderTransformInput, ProviderTransformLoss};
use crate::{ProviderEndpoint, ProviderId};
use serde_json::{Value, json};

fn assert_stream_event(value: Value, expected: Option<&str>) {
    let data = serde_json::to_string(&value).expect("event JSON");
    let event = format!("data: {data}\n\n");
    let actual = deepseek_transform_stream_event(
        ProviderId::DeepSeek,
        ProviderTransformInput::new(ProviderEndpoint::Responses, event.as_bytes()),
    );
    match expected {
        Some(expected) => {
            assert_eq!(actual.loss, ProviderTransformLoss::Lossless, "event={data}");
            assert_eq!(
                actual.body.as_deref(),
                Some(expected.as_bytes()),
                "event={data}"
            );
        }
        None => {
            assert_eq!(
                actual.loss,
                ProviderTransformLoss::UnsupportedUpstream {
                    reason:
                        "DeepSeek SSE event does not contain a supported text or function-call delta"
                            .into(),
                },
                "event={data}"
            );
            assert!(actual.body.is_none(), "event={data}");
        }
    }
}

#[test]
fn deepseek_stream_events_match_golden_shapes() {
    let empty_text = "event: response.output_text.delta\ndata: {\"delta\":\"\",\"type\":\"response.output_text.delta\"}\n\n";
    for (value, expected) in [
        (json!({}), None),
        (json!([]), None),
        (json!({"choices": []}), None),
        (json!({"choices": [null]}), None),
        (
            json!({"choices": [{"delta": {}, "finish_reason": null}, {"delta": {"content": "ignored second choice"}}]}),
            Some(empty_text),
        ),
        (json!({"choices": [{"delta": null}]}), Some(empty_text)),
        (json!({"choices": [{"delta": {}}]}), Some(empty_text)),
        (
            json!({"choices": [{"delta": {"content": ""}}]}),
            Some(empty_text),
        ),
        (
            json!({"choices": [{"delta": {"content": 42}}]}),
            Some(empty_text),
        ),
        (
            json!({"choices": [{"delta": {"content": "東京\n\"quoted\""}, "finish_reason": "stop"}]}),
            Some(
                r#"event: response.output_text.delta
data: {"delta":"東京\n\"quoted\"","type":"response.output_text.delta"}

"#,
            ),
        ),
        (
            json!({"choices": [{"delta": {"tool_calls": [], "content": "text"}}]}),
            Some(
                "event: response.output_text.delta\ndata: {\"delta\":\"text\",\"type\":\"response.output_text.delta\"}\n\n",
            ),
        ),
        (
            json!({"choices": [{"delta": {"tool_calls": [{"id": "call-1", "function": {"name": "shell", "arguments": "{\"cmd\":\"ls\"}"}}], "content": "lower priority"}}]}),
            Some(
                r#"event: response.function_call_arguments.delta
data: {"call_id":"call-1","delta":"{\"cmd\":\"ls\"}","type":"response.function_call_arguments.delta"}

"#,
            ),
        ),
        (
            json!({"choices": [{"delta": {"tool_calls": [{"id": false, "function": {"name": 7, "arguments": "raw"}}], "content": "lower priority"}}]}),
            Some(
                "event: response.function_call_arguments.delta\ndata: {\"delta\":\"raw\",\"type\":\"response.function_call_arguments.delta\"}\n\n",
            ),
        ),
        (
            json!({"choices": [{"delta": {"tool_calls": [{"function": {"arguments": false}}], "content": "no text fallback"}}]}),
            None,
        ),
        (
            json!({"choices": [{"delta": {"tool_calls": [{"function": {}}], "content": "no text fallback"}}]}),
            None,
        ),
    ] {
        assert_stream_event(value, expected);
    }
}

#[test]
fn deepseek_done_and_sse_errors_keep_transport_boundary_behavior() {
    let done = deepseek_transform_stream_event(
        ProviderId::DeepSeek,
        ProviderTransformInput::new(ProviderEndpoint::Responses, b"data: [DONE]\n\n"),
    );
    assert_eq!(done.loss, ProviderTransformLoss::Lossless);
    assert_eq!(
        done.body.as_deref(),
        Some(b"event: response.completed\ndata: {}\n\n".as_slice())
    );

    let invalid_framing = deepseek_transform_stream_event(
        ProviderId::DeepSeek,
        ProviderTransformInput::new(ProviderEndpoint::Responses, b"data:{}\n\n"),
    );
    assert_eq!(
        invalid_framing.loss,
        ProviderTransformLoss::UnsupportedUpstream {
            reason: "DeepSeek SSE event must use data: <json> framing".into(),
        }
    );

    let invalid_json = deepseek_transform_stream_event(
        ProviderId::DeepSeek,
        ProviderTransformInput::new(ProviderEndpoint::Responses, b"data: {bad}\n\n"),
    );
    assert!(matches!(
        invalid_json.loss,
        ProviderTransformLoss::Rejected { reason }
            if reason.starts_with("failed to parse DeepSeek SSE JSON: ")
    ));
}
