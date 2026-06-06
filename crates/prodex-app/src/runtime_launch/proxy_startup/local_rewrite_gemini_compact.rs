use crate::RuntimeHeapTrimmedBufferedResponseParts;

const GEMINI_LOCAL_COMPACT_SUMMARY_PREFIX: &str = "Another language model started to solve this problem and produced a summary of its thinking process. You also have access to the state of the tools that were used by that language model. Use this to build on the work that has already been done and avoid duplicating work. Here is the summary produced by the other language model, use the information in this summary to assist with your own analysis:";
const GEMINI_LOCAL_COMPACT_MAX_SNIPPETS: usize = 24;
const GEMINI_LOCAL_COMPACT_MAX_SNIPPET_BYTES: usize = 768;
const GEMINI_LOCAL_COMPACT_MAX_SUMMARY_BYTES: usize = 24 * 1024;

pub(super) fn runtime_gemini_local_compact_response_parts(
    body: &[u8],
) -> RuntimeHeapTrimmedBufferedResponseParts {
    let summary = runtime_gemini_local_compact_summary(body);
    let text = format!("{GEMINI_LOCAL_COMPACT_SUMMARY_PREFIX}\n\n{summary}");
    let body = serde_json::to_vec(&serde_json::json!({
        "output": [{
            "type": "message",
            "role": "user",
            "content": [{
                "type": "input_text",
                "text": text,
            }],
        }],
    }))
    .unwrap_or_else(|_| b"{\"output\":[]}".to_vec());

    RuntimeHeapTrimmedBufferedResponseParts {
        status: 200,
        headers: vec![(
            "content-type".to_string(),
            b"application/json; charset=utf-8".to_vec(),
        )],
        body: body.into(),
    }
}

fn runtime_gemini_local_compact_summary(body: &[u8]) -> String {
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(body) else {
        return "Local Gemini compact fallback could not parse the compact request body."
            .to_string();
    };

    let model = value
        .get("model")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|model| !model.is_empty())
        .unwrap_or("unknown");
    let input = value
        .get("input")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();

    let mut snippets = input
        .iter()
        .filter_map(runtime_gemini_local_compact_snippet)
        .collect::<Vec<_>>();
    if snippets.len() > GEMINI_LOCAL_COMPACT_MAX_SNIPPETS {
        snippets = snippets.split_off(snippets.len() - GEMINI_LOCAL_COMPACT_MAX_SNIPPETS);
    }

    let mut summary = String::new();
    summary.push_str("Local Gemini compact fallback summary.\n\n");
    summary.push_str(&format!("Model: {model}\n"));
    summary.push_str(&format!("Original input items: {}\n", input.len()));
    summary.push_str(&format!("Retained recent items: {}\n\n", snippets.len()));
    summary.push_str("Recent conversation and tool state:\n");

    if snippets.is_empty() {
        summary.push_str("- No parseable recent message or tool content was found.\n");
    } else {
        for snippet in snippets {
            summary.push_str("- ");
            summary.push_str(&snippet.replace('\n', "\n  "));
            summary.push('\n');
        }
    }

    truncate_utf8(summary, GEMINI_LOCAL_COMPACT_MAX_SUMMARY_BYTES)
}

fn runtime_gemini_local_compact_snippet(item: &serde_json::Value) -> Option<String> {
    let object = item.as_object()?;
    let item_type = object
        .get("type")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("item");
    let snippet = match item_type {
        "message" => {
            let role = object
                .get("role")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("unknown");
            let text = object
                .get("content")
                .and_then(runtime_gemini_local_compact_text_from_content)
                .or_else(|| {
                    object
                        .get("text")
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_string)
                })
                .unwrap_or_default();
            if text.trim().is_empty() {
                format!("{role} message with no text content")
            } else {
                format!(
                    "{role} message: {}",
                    truncate_utf8(text, GEMINI_LOCAL_COMPACT_MAX_SNIPPET_BYTES)
                )
            }
        }
        "function_call" => {
            let name = object
                .get("name")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("function");
            let call_id = object
                .get("call_id")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("unknown");
            let arguments = object
                .get("arguments")
                .and_then(runtime_gemini_local_compact_text_from_content)
                .unwrap_or_default();
            format!(
                "tool call {name} ({call_id}): {}",
                truncate_utf8(arguments, GEMINI_LOCAL_COMPACT_MAX_SNIPPET_BYTES)
            )
        }
        "custom_tool_call" => {
            let name = object
                .get("name")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("custom_tool");
            let call_id = object
                .get("call_id")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("unknown");
            let input = object
                .get("input")
                .and_then(runtime_gemini_local_compact_text_from_content)
                .unwrap_or_default();
            format!(
                "custom tool call {name} ({call_id}): {}",
                truncate_utf8(input, GEMINI_LOCAL_COMPACT_MAX_SNIPPET_BYTES)
            )
        }
        "function_call_output" | "custom_tool_call_output" => {
            let call_id = object
                .get("call_id")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("unknown");
            let output = object
                .get("output")
                .and_then(runtime_gemini_local_compact_text_from_content)
                .unwrap_or_default();
            format!(
                "tool output {call_id}: {}",
                truncate_utf8(output, GEMINI_LOCAL_COMPACT_MAX_SNIPPET_BYTES)
            )
        }
        "local_shell_call" => {
            let call_id = object
                .get("call_id")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("unknown");
            let action = object
                .get("action")
                .and_then(runtime_gemini_local_compact_text_from_content)
                .unwrap_or_default();
            format!(
                "local shell call {call_id}: {}",
                truncate_utf8(action, GEMINI_LOCAL_COMPACT_MAX_SNIPPET_BYTES)
            )
        }
        "web_search_call" => {
            let action = object
                .get("action")
                .and_then(runtime_gemini_local_compact_text_from_content)
                .unwrap_or_default();
            format!(
                "web search: {}",
                truncate_utf8(action, GEMINI_LOCAL_COMPACT_MAX_SNIPPET_BYTES)
            )
        }
        "reasoning" => {
            let summary = object
                .get("summary")
                .and_then(runtime_gemini_local_compact_text_from_content)
                .unwrap_or_default();
            if summary.trim().is_empty() {
                return None;
            }
            format!(
                "reasoning summary: {}",
                truncate_utf8(summary, GEMINI_LOCAL_COMPACT_MAX_SNIPPET_BYTES)
            )
        }
        _ => {
            let text = runtime_gemini_local_compact_text_from_content(item).unwrap_or_default();
            if text.trim().is_empty() {
                return None;
            }
            format!(
                "{item_type}: {}",
                truncate_utf8(text, GEMINI_LOCAL_COMPACT_MAX_SNIPPET_BYTES)
            )
        }
    };

    Some(snippet)
}

fn runtime_gemini_local_compact_text_from_content(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(text) => Some(text.to_string()),
        serde_json::Value::Array(values) => {
            let text = values
                .iter()
                .filter_map(runtime_gemini_local_compact_text_from_content)
                .filter(|text| !text.trim().is_empty())
                .collect::<Vec<_>>()
                .join("\n");
            (!text.trim().is_empty()).then_some(text)
        }
        serde_json::Value::Object(object) => {
            for key in ["text", "output", "input", "query", "command", "commands"] {
                if let Some(text) = object
                    .get(key)
                    .and_then(runtime_gemini_local_compact_text_from_content)
                    .filter(|text| !text.trim().is_empty())
                {
                    return Some(text);
                }
            }
            let text = object
                .values()
                .filter_map(runtime_gemini_local_compact_text_from_content)
                .filter(|text| !text.trim().is_empty())
                .collect::<Vec<_>>()
                .join("\n");
            (!text.trim().is_empty()).then_some(text)
        }
        serde_json::Value::Number(number) => Some(number.to_string()),
        serde_json::Value::Bool(value) => Some(value.to_string()),
        serde_json::Value::Null => None,
    }
}

fn truncate_utf8(mut text: String, max_bytes: usize) -> String {
    if text.len() <= max_bytes {
        return text;
    }
    let mut end = max_bytes.min(text.len());
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    text.truncate(end);
    text.push_str("\n[truncated]");
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gemini_local_compact_returns_codex_compact_output() {
        let body = serde_json::to_vec(&serde_json::json!({
            "model": "auto",
            "input": [
                {
                    "type": "message",
                    "role": "user",
                    "content": [{"type": "input_text", "text": "Investigate Gemini compact."}]
                },
                {
                    "type": "function_call",
                    "name": "shell",
                    "call_id": "call_1",
                    "arguments": "{\"cmd\":\"cargo test\"}"
                },
                {
                    "type": "function_call_output",
                    "call_id": "call_1",
                    "output": "tests passed"
                }
            ]
        }))
        .unwrap();

        let parts = runtime_gemini_local_compact_response_parts(&body);
        let value: serde_json::Value = serde_json::from_slice(&parts.body).unwrap();
        let text = value["output"][0]["content"][0]["text"].as_str().unwrap();

        assert_eq!(parts.status, 200);
        assert_eq!(value["output"][0]["type"], "message");
        assert_eq!(value["output"][0]["role"], "user");
        assert!(text.contains(GEMINI_LOCAL_COMPACT_SUMMARY_PREFIX));
        assert!(text.contains("Investigate Gemini compact."));
        assert!(text.contains("tool call shell (call_1)"));
        assert!(text.contains("tests passed"));
    }

    #[test]
    fn gemini_local_compact_bounds_large_history() {
        let input = (0..40)
            .map(|index| {
                serde_json::json!({
                    "type": "message",
                    "role": "user",
                    "content": [{"type": "input_text", "text": format!("item {index} {}", "x".repeat(10_000))}]
                })
            })
            .collect::<Vec<_>>();
        let body = serde_json::to_vec(&serde_json::json!({
            "model": "auto",
            "input": input,
        }))
        .unwrap();

        let parts = runtime_gemini_local_compact_response_parts(&body);
        let value: serde_json::Value = serde_json::from_slice(&parts.body).unwrap();
        let text = value["output"][0]["content"][0]["text"].as_str().unwrap();

        assert!(
            text.len()
                <= GEMINI_LOCAL_COMPACT_SUMMARY_PREFIX.len()
                    + 2
                    + GEMINI_LOCAL_COMPACT_MAX_SUMMARY_BYTES
                    + 16
        );
        assert!(!text.contains("item 0 "));
        assert!(text.contains("item 39 "));
        assert!(text.contains("[truncated]"));
    }
}
