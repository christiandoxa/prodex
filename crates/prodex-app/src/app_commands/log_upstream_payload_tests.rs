use super::log_upstream_payload::render_upstream_payload_lines;
use serde_json::json;

#[test]
fn payload_projection_preserves_summary_metadata_content_and_tool_precedence() {
    let payload = json!({
        "model": "synthetic", "stream": true, "store": false,
        "tool_choice": "auto", "parallel_tool_calls": true,
        "prompt_cache_key": "key", "reasoning": {"effort": "high"},
        "previous_response_id": null,
        "client_metadata": {"session_id": "s", "context_window_id": "abcd-rest"},
        "instructions": "follow instructions",
        "input": [
            {"role": "user", "content": [
                {"type": "input_text", "text": "hello 月🙂"},
                {"type": "input_image", "text": "image"},
                {"type": "input_audio", "text": "audio"}
            ]},
            {"type": "function_call", "name": "lookup", "arguments": "{\"q\":1}"},
            {"type": "function_call_output", "output": "plain output"},
            {"type": "compaction", "text": "summary"}
        ],
        "tools": [{"name": "outer", "function": {"name": "ignored"}},
                  {"function": {"name": "nested"}}, null]
    });
    assert_eq!(
        render_upstream_payload_lines(&payload.to_string(), 512),
        [
            "model=synthetic stream=true store=false tool_choice=auto parallel_tool_calls=true prompt_cache_key=key effort=high input_items=4 images=1 audio=1 tools=3 continuation=previous_response compaction=yes",
            "client_metadata: session_id=s ctx=abcd",
            "instructions:", "  follow instructions",
            "input[0] user:", "  hello 月🙂", "  image", "  audio",
            "input[1] tool-call:lookup:", "  {", "  \"q\": 1", "  }",
            "input[2] tool-output:", "  plain output",
            "input[3] compaction:", "  summary", "tools: 3 outer, nested"
        ].map(str::to_string)
    );
}

#[test]
fn payload_projection_keeps_unicode_byte_thresholds_and_json_presence() {
    for (id, expected) in [("ab🦀c", "ab🦀c"), ("🦀abc", "🦀"), ("abc", "abc")] {
        let payload = json!({"client_metadata": {"context_window_id": id}});
        assert_eq!(
            render_upstream_payload_lines(&payload.to_string(), 120),
            [format!("client_metadata: ctx={expected}")]
        );
    }
    let model = "β".repeat(41);
    assert_eq!(
        render_upstream_payload_lines(&json!({"model":model}).to_string(), 512),
        [format!("model={}...", "β".repeat(41))]
    );
    for (payload, expected) in [
        (
            json!({"input":[{"content":{"text":null,"content":"ignored"},"text":""}]}),
            vec!["input_items=1", "input[0] message:", "  null"],
        ),
        (
            json!({"input":[{"content":["", " \n", {"input_text":"visible"}]}]}),
            vec!["input_items=1", "input[0] message:", "  visible"],
        ),
        (
            json!({"input":[{"type":"function_call"}]}),
            vec!["input_items=1", "input[0] tool-call:tool:", "  "],
        ),
        (json!({"tools":[]}), vec!["tools=0", "tools: 0"]),
    ] {
        assert_eq!(
            render_upstream_payload_lines(&payload.to_string(), 512),
            expected
        );
    }
}

#[test]
fn payload_projection_preserves_plain_and_unrecognized_json_rendering_and_narrow_wrap() {
    for text in ["not JSON", "null", "{}", "[]", "\"quoted\"", "[1,true]"] {
        let expected = serde_json::from_str::<serde_json::Value>(text)
            .ok()
            .map(|value| serde_json::to_string_pretty(&value).unwrap())
            .unwrap_or_else(|| text.to_string());
        assert_eq!(
            render_upstream_payload_lines(text, 120),
            super::log_format::render_text_body(&expected, 120)
        );
    }
    assert_eq!(
        render_upstream_payload_lines(r#"{"input":"alpha beta gamma delta epsilon"}"#, 1),
        ["input:", "  alpha beta gamma", "  delta epsilon"]
    );
}
