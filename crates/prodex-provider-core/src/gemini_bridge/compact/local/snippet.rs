//! Gemini compact transcript snippet extraction.

use super::{
    GEMINI_PROVIDER_CORE_LOCAL_COMPACT_MAX_SNIPPET_BYTES,
    gemini_provider_core_local_compact_text_from_content,
};
use prodex_mojo_core::rich::{GeminiCompactSnippetInput, format_gemini_compact_snippet};

pub(super) fn gemini_provider_core_local_compact_snippet(
    item: &serde_json::Value,
) -> Option<String> {
    let object = item.as_object()?;
    let item_type = object.get("type").and_then(serde_json::Value::as_str);
    let role = object.get("role").and_then(serde_json::Value::as_str);
    let name = object.get("name").and_then(serde_json::Value::as_str);
    let call_id = object.get("call_id").and_then(serde_json::Value::as_str);
    let content = object
        .get("content")
        .and_then(gemini_provider_core_local_compact_text_from_content);
    let text = object.get("text").and_then(serde_json::Value::as_str);
    let arguments = object
        .get("arguments")
        .and_then(gemini_provider_core_local_compact_text_from_content);
    let tool_input = object
        .get("input")
        .and_then(gemini_provider_core_local_compact_text_from_content);
    let tool_output = object
        .get("output")
        .and_then(gemini_provider_core_local_compact_text_from_content);
    let action = object
        .get("action")
        .and_then(gemini_provider_core_local_compact_text_from_content);
    let summary = object
        .get("summary")
        .and_then(gemini_provider_core_local_compact_text_from_content);
    let generic = gemini_provider_core_local_compact_text_from_content(item);

    format_gemini_compact_snippet(
        GeminiCompactSnippetInput {
            item_type,
            role,
            name,
            call_id,
            content: content.as_deref(),
            text,
            arguments: arguments.as_deref(),
            tool_input: tool_input.as_deref(),
            tool_output: tool_output.as_deref(),
            action: action.as_deref(),
            summary: summary.as_deref(),
            generic: generic.as_deref(),
        },
        GEMINI_PROVIDER_CORE_LOCAL_COMPACT_MAX_SNIPPET_BYTES,
    )
    .expect("Mojo Gemini compact snippet formatter returned invalid output")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn complete_snippet_stays_within_its_byte_limit() {
        let snippet = gemini_provider_core_local_compact_snippet(&serde_json::json!({
            "type": "message",
            "role": "user",
            "content": [{"type": "input_text", "text": "月".repeat(1_000)}],
        }))
        .unwrap();
        assert!(snippet.len() <= GEMINI_PROVIDER_CORE_LOCAL_COMPACT_MAX_SNIPPET_BYTES);
        assert!(snippet.ends_with("\n[truncated]"));
    }

    #[test]
    fn compact_snippet_shapes_are_mojo_owned_at_provider_boundary() {
        let cases = [
            (
                serde_json::json!({"type":"message","role":"assistant","content":"hello"}),
                Some("assistant message: hello"),
            ),
            (
                serde_json::json!({"type":"message","content":"   ","text":"ignored"}),
                Some("unknown message with no text content"),
            ),
            (
                serde_json::json!({"type":"function_call","arguments":{"query":"snow"}}),
                Some("tool call function (unknown): snow"),
            ),
            (
                serde_json::json!({"type":"custom_tool_call","name":"patch","call_id":"c1","input":"x"}),
                Some("custom tool call patch (c1): x"),
            ),
            (
                serde_json::json!({"type":"function_call_output","call_id":"c2","output":true}),
                Some("tool output c2: true"),
            ),
            (
                serde_json::json!({"type":"local_shell_call","call_id":"c3","action":{"command":"pwd"}}),
                Some("local shell call c3: pwd"),
            ),
            (
                serde_json::json!({"type":"web_search_call","action":{"query":"mojo"}}),
                Some("web search: mojo"),
            ),
            (
                serde_json::json!({"type":"reasoning","summary":"think"}),
                Some("reasoning summary: think"),
            ),
            (serde_json::json!({"type":"reasoning","summary":"  "}), None),
            (
                serde_json::json!({"type":"other","text":"value"}),
                Some("other: value"),
            ),
        ];
        for (value, expected) in cases {
            assert_eq!(
                gemini_provider_core_local_compact_snippet(&value).as_deref(),
                expected
            );
        }
    }
}
