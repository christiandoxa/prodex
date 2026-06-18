use super::gemini_rewrite::{
    RuntimeGeminiProviderAuth, runtime_gemini_normalized_response_value,
    runtime_gemini_responses_value_from_generate_value,
};
use super::local_rewrite::{
    RUNTIME_LOCAL_REWRITE_PROFILE, RuntimeLocalRewriteProxyShared,
    RuntimeLocalRewriteUpstreamResponse, RuntimeLocalRewriteUpstreamResult,
};
use super::local_rewrite_gemini::send_runtime_gemini_upstream_request;
use super::local_rewrite_response::runtime_local_rewrite_response_with_call_id;
use super::*;
use crate::RuntimeHeapTrimmedBufferedResponseParts;
use anyhow::{Context, Result, bail};
use prodex_runtime_gemini::GEMINI_CHAT_COMPRESSION_MODEL;
use std::io::Read;

const GEMINI_LOCAL_COMPACT_SUMMARY_PREFIX: &str = "Another language model started to solve this problem and produced a summary of its thinking process. You also have access to the state of the tools that were used by that language model. Use this to build on the work that has already been done and avoid duplicating work. Here is the summary produced by the other language model, use the information in this summary to assist with your own analysis:";
const GEMINI_SEMANTIC_COMPACT_INSTRUCTIONS: &str = "\
Compact the supplied coding-agent transcript into one durable continuation summary. \
Preserve the user's goals, repository instructions, decisions, files changed, exact identifiers, \
commands and test results, unresolved failures, current worktree state, and the next concrete steps. \
Remove redundant narration and obsolete intermediate reasoning. Do not call tools. \
Return only the continuation summary, with no preamble or completion claim.";
const GEMINI_LOCAL_COMPACT_MAX_SNIPPETS: usize = 24;
const GEMINI_LOCAL_COMPACT_MAX_SNIPPET_BYTES: usize = 768;
const GEMINI_LOCAL_COMPACT_MAX_SUMMARY_BYTES: usize = 24 * 1024;
const GEMINI_SEMANTIC_COMPACT_ACTIVE_USER_MAX_BYTES: usize = 2 * 1024;
const GEMINI_SEMANTIC_COMPACT_LATEST_TOOL_MAX_BYTES: usize = 1024;

pub(super) fn respond_runtime_gemini_compact_request(
    request_id: u64,
    request: tiny_http::Request,
    captured: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
    auth: &RuntimeGeminiProviderAuth,
) {
    let semantic = runtime_gemini_semantic_compact_request_body(&captured.body).and_then(|body| {
        let semantic_request = RuntimeProxyRequest {
            method: captured.method.clone(),
            path_and_query: format!("{}/responses", shared.mount_path.trim_end_matches('/')),
            headers: captured.headers.clone(),
            body: body.clone(),
        };
        let result = send_runtime_gemini_upstream_request(
            request_id,
            &semantic_request,
            shared,
            body,
            auth,
        )?;
        let RuntimeLocalRewriteUpstreamResult {
            response,
            gemini_context,
            ..
        } = result;
        let profile_name = gemini_context
            .as_ref()
            .map(|context| context.profile_name.as_str())
            .unwrap_or(RUNTIME_LOCAL_REWRITE_PROFILE);
        let parts = match response {
            RuntimeLocalRewriteUpstreamResponse::Live(live) if live.prefix.is_empty() => {
                let status = live.response.status().as_u16();
                runtime_gemini_semantic_compact_response_parts(
                    status,
                    live.response,
                    request_id,
                    &captured.body,
                )?
            }
            RuntimeLocalRewriteUpstreamResponse::Live(_) => {
                bail!("Gemini semantic compact unexpectedly returned a stream prefix")
            }
            RuntimeLocalRewriteUpstreamResponse::Buffered(parts) => {
                bail!(
                    "Gemini semantic compact returned buffered HTTP {}",
                    parts.status
                )
            }
        };
        Ok((parts, profile_name.to_string()))
    });

    let parts = match semantic {
        Ok((parts, profile_name)) => {
            runtime_proxy_log(
                &shared.runtime_shared,
                runtime_proxy_structured_log_message(
                    "local_rewrite_gemini_compact_semantic",
                    [
                        runtime_proxy_log_field("request", request_id.to_string()),
                        runtime_proxy_log_field("transport", "http"),
                        runtime_proxy_log_field("profile", profile_name),
                        runtime_proxy_log_field(
                            "path",
                            path_without_query(&captured.path_and_query),
                        ),
                        runtime_proxy_log_field("body_bytes", captured.body.len().to_string()),
                    ],
                ),
            );
            parts
        }
        Err(err) => {
            runtime_proxy_log(
                &shared.runtime_shared,
                runtime_proxy_structured_log_message(
                    "local_rewrite_gemini_compact_fallback",
                    [
                        runtime_proxy_log_field("request", request_id.to_string()),
                        runtime_proxy_log_field("transport", "http"),
                        runtime_proxy_log_field(
                            "path",
                            path_without_query(&captured.path_and_query),
                        ),
                        runtime_proxy_log_field("body_bytes", captured.body.len().to_string()),
                        runtime_proxy_log_field("reason", format!("{err:#}")),
                    ],
                ),
            );
            runtime_gemini_local_compact_response_parts(&captured.body)
        }
    };
    let _ = request.respond(runtime_local_rewrite_response_with_call_id(
        parts, request_id, shared,
    ));
}

pub(super) fn runtime_gemini_semantic_compact_request_body(body: &[u8]) -> Result<Vec<u8>> {
    let mut value: serde_json::Value =
        serde_json::from_slice(body).context("failed to parse Gemini compact request JSON")?;
    let object = value
        .as_object_mut()
        .context("Gemini compact request must be a JSON object")?;
    let input = object
        .get_mut("input")
        .and_then(serde_json::Value::as_array_mut)
        .context("Gemini compact request must contain an input array")?;
    input.push(serde_json::json!({
        "type": "message",
        "role": "user",
        "content": [{
            "type": "input_text",
            "text": GEMINI_SEMANTIC_COMPACT_INSTRUCTIONS,
        }],
    }));

    object.insert(
        "instructions".to_string(),
        serde_json::Value::String(GEMINI_SEMANTIC_COMPACT_INSTRUCTIONS.to_string()),
    );
    object.insert(
        "model".to_string(),
        serde_json::Value::String(GEMINI_CHAT_COMPRESSION_MODEL.to_string()),
    );
    object.insert("stream".to_string(), serde_json::Value::Bool(false));
    object.insert("store".to_string(), serde_json::Value::Bool(false));
    object.insert(
        "parallel_tool_calls".to_string(),
        serde_json::Value::Bool(false),
    );
    object.insert(
        "prodex_gemini_compaction".to_string(),
        serde_json::Value::Bool(true),
    );
    for key in [
        "include",
        "previous_response_id",
        "prompt_cache_key",
        "text",
        "tool_choice",
        "tools",
    ] {
        object.remove(key);
    }

    serde_json::to_vec(&value).context("failed to serialize Gemini semantic compact request")
}

pub(super) fn runtime_gemini_semantic_compact_response_parts(
    status: u16,
    mut response: reqwest::blocking::Response,
    request_id: u64,
    compact_request_body: &[u8],
) -> Result<RuntimeHeapTrimmedBufferedResponseParts> {
    let mut body = Vec::new();
    response
        .read_to_end(&mut body)
        .context("failed to read Gemini semantic compact response")?;
    let value: serde_json::Value = serde_json::from_slice(&body)
        .context("failed to parse Gemini semantic compact response")?;
    runtime_gemini_semantic_compact_response_parts_from_value(
        status,
        &value,
        request_id,
        compact_request_body,
    )
}

fn runtime_gemini_semantic_compact_response_parts_from_value(
    status: u16,
    value: &serde_json::Value,
    request_id: u64,
    compact_request_body: &[u8],
) -> Result<RuntimeHeapTrimmedBufferedResponseParts> {
    if !(200..300).contains(&status) {
        bail!("Gemini semantic compact returned HTTP {status}");
    }
    let summary = runtime_gemini_semantic_compact_summary(value, request_id);
    if summary.is_empty() {
        bail!("Gemini semantic compact returned no summary text");
    }
    let summary =
        runtime_gemini_semantic_compact_continuation_summary(&summary, compact_request_body);
    Ok(runtime_gemini_compact_response_parts(&summary))
}

fn runtime_gemini_semantic_compact_summary(value: &serde_json::Value, request_id: u64) -> String {
    if let Some(content) = value
        .get("choices")
        .and_then(serde_json::Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("content"))
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
    {
        return content.to_string();
    }

    let value = runtime_gemini_normalized_response_value(value);
    let response = runtime_gemini_responses_value_from_generate_value(&value, request_id);
    response
        .get("output")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter(|item| item.get("type").and_then(serde_json::Value::as_str) == Some("message"))
        .flat_map(|item| {
            item.get("content")
                .and_then(serde_json::Value::as_array)
                .into_iter()
                .flatten()
        })
        .filter(|item| {
            matches!(
                item.get("type").and_then(serde_json::Value::as_str),
                Some("output_text" | "input_text")
            )
        })
        .filter_map(|item| item.get("text").and_then(serde_json::Value::as_str))
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn runtime_gemini_semantic_compact_continuation_summary(
    semantic_summary: &str,
    compact_request_body: &[u8],
) -> String {
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(compact_request_body) else {
        return semantic_summary.trim().to_string();
    };
    let input = value
        .get("input")
        .and_then(serde_json::Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    let active_user_index = input.iter().rposition(|item| {
        item.get("type").and_then(serde_json::Value::as_str) == Some("message")
            && item.get("role").and_then(serde_json::Value::as_str) == Some("user")
    });
    let active_user = active_user_index
        .and_then(|index| {
            input[index]
                .get("content")
                .and_then(runtime_gemini_local_compact_text_from_content)
        })
        .filter(|text| !text.trim().is_empty())
        .map(|text| truncate_utf8_edges(text, GEMINI_SEMANTIC_COMPACT_ACTIVE_USER_MAX_BYTES));
    let latest_tool = active_user_index
        .and_then(|index| {
            input[(index + 1)..].iter().rev().find(|item| {
                matches!(
                    item.get("type").and_then(serde_json::Value::as_str),
                    Some(
                        "function_call_output"
                            | "custom_tool_call_output"
                            | "local_shell_call_output"
                    )
                )
            })
        })
        .and_then(|item| {
            item.get("output")
                .or_else(|| item.get("content"))
                .and_then(runtime_gemini_local_compact_text_from_content)
        })
        .filter(|text| !text.trim().is_empty())
        .map(|text| truncate_utf8_edges(text, GEMINI_SEMANTIC_COMPACT_LATEST_TOOL_MAX_BYTES));

    let mut summary = String::new();
    if let Some(active_user) = active_user {
        summary.push_str("Active user request that must still be completed:\n");
        summary.push_str(active_user.trim());
        summary.push_str("\n\n");
    }
    if let Some(latest_tool) = latest_tool {
        summary.push_str("Latest tool result after the active request:\n");
        summary.push_str(latest_tool.trim());
        summary.push_str("\n\n");
    }
    summary.push_str("Semantic continuation summary:\n");
    summary.push_str(semantic_summary.trim());
    summary.push_str(
        "\n\nContinue the active user request. Do not merely acknowledge repository, optimizer, or environment instructions.",
    );
    truncate_utf8(summary, GEMINI_LOCAL_COMPACT_MAX_SUMMARY_BYTES)
}

pub(super) fn runtime_gemini_local_compact_response_parts(
    body: &[u8],
) -> RuntimeHeapTrimmedBufferedResponseParts {
    let summary = runtime_gemini_local_compact_summary(body);
    runtime_gemini_compact_response_parts(&summary)
}

fn runtime_gemini_compact_response_parts(summary: &str) -> RuntimeHeapTrimmedBufferedResponseParts {
    let text = format!(
        "{GEMINI_LOCAL_COMPACT_SUMMARY_PREFIX}\n\n{}",
        summary.trim()
    );
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

fn truncate_utf8_edges(text: String, max_bytes: usize) -> String {
    if text.len() <= max_bytes {
        return text;
    }
    let head_bytes = max_bytes / 3;
    let tail_bytes = max_bytes.saturating_sub(head_bytes);
    let mut head_end = head_bytes.min(text.len());
    while head_end > 0 && !text.is_char_boundary(head_end) {
        head_end -= 1;
    }
    let mut tail_start = text.len().saturating_sub(tail_bytes);
    while tail_start < text.len() && !text.is_char_boundary(tail_start) {
        tail_start += 1;
    }
    format!(
        "{}\n[... middle truncated ...]\n{}",
        &text[..head_end],
        &text[tail_start..]
    )
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

    #[test]
    fn gemini_semantic_compact_request_disables_tools_and_streaming() {
        let body = serde_json::to_vec(&serde_json::json!({
            "model": "auto",
            "instructions": "Act as a coding agent.",
            "input": [{
                "type": "message",
                "role": "user",
                "content": [{"type": "input_text", "text": "Investigate the failure."}]
            }],
            "tools": [{"type": "function", "name": "shell"}],
            "tool_choice": "auto",
            "parallel_tool_calls": true,
            "stream": true,
            "text": {"format": {"type": "json_schema"}},
        }))
        .unwrap();

        let translated = runtime_gemini_semantic_compact_request_body(&body).unwrap();
        let value: serde_json::Value = serde_json::from_slice(&translated).unwrap();
        let input = value["input"].as_array().unwrap();

        assert_eq!(value["stream"], false);
        assert_eq!(value["store"], false);
        assert_eq!(value["parallel_tool_calls"], false);
        assert_eq!(value["model"], GEMINI_CHAT_COMPRESSION_MODEL);
        assert_eq!(value["prodex_gemini_compaction"], true);
        assert!(value.get("tools").is_none());
        assert!(value.get("tool_choice").is_none());
        assert!(value.get("text").is_none());
        assert_eq!(input.len(), 2);
        assert!(
            input[1]["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("durable continuation summary")
        );
    }

    #[test]
    fn gemini_semantic_compact_returns_codex_replacement_history() {
        let request = serde_json::to_vec(&serde_json::json!({
            "input": [
                {
                    "type": "message",
                    "role": "user",
                    "content": [{"type": "input_text", "text": "Complete the compact fix."}]
                },
                {
                    "type": "function_call_output",
                    "call_id": "call_test",
                    "output": "focused tests passed"
                }
            ]
        }))
        .unwrap();
        let value = serde_json::json!({
            "responseId": "resp_compact",
            "modelVersion": "gemini-2.5-pro",
            "candidates": [{
                "content": {
                    "parts": [{"text": "Goal: fix compact.\nTests: cargo test passed."}]
                },
                "finishReason": "STOP"
            }]
        });

        let parts =
            runtime_gemini_semantic_compact_response_parts_from_value(200, &value, 99, &request)
                .unwrap();
        let value: serde_json::Value = serde_json::from_slice(&parts.body).unwrap();
        let text = value["output"][0]["content"][0]["text"].as_str().unwrap();

        assert_eq!(parts.status, 200);
        assert_eq!(value["output"][0]["role"], "user");
        assert!(text.contains(GEMINI_LOCAL_COMPACT_SUMMARY_PREFIX));
        assert!(text.contains("Active user request that must still be completed"));
        assert!(text.contains("Complete the compact fix."));
        assert!(text.contains("Latest tool result after the active request"));
        assert!(text.contains("focused tests passed"));
        assert!(text.contains("Goal: fix compact."));
        assert!(text.contains("Tests: cargo test passed."));
        assert!(text.contains("Continue the active user request."));
    }

    #[test]
    fn gemini_semantic_compact_rejects_empty_output() {
        let value = serde_json::json!({
            "responseId": "resp_compact_empty",
            "modelVersion": "gemini-2.5-pro",
            "candidates": [{
                "content": {"parts": []},
                "finishReason": "STOP"
            }]
        });

        let error = match runtime_gemini_semantic_compact_response_parts_from_value(
            200,
            &value,
            100,
            b"{\"input\":[]}",
        ) {
            Ok(_) => panic!("empty semantic compact output should fail"),
            Err(error) => error,
        };

        assert!(error.to_string().contains("no summary text"));
    }

    #[test]
    fn gemini_semantic_compact_accepts_openai_chat_completion_output() {
        let value = serde_json::json!({
            "id": "chatcmpl_compact",
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": "Goal: keep the Gemini OpenAI-compatible adapter working."
                },
                "finish_reason": "stop"
            }]
        });

        let parts = runtime_gemini_semantic_compact_response_parts_from_value(
            200,
            &value,
            101,
            b"{\"input\":[]}",
        )
        .unwrap();
        let response: serde_json::Value = serde_json::from_slice(&parts.body).unwrap();
        let text = response["output"][0]["content"][0]["text"]
            .as_str()
            .unwrap();

        assert!(text.contains("Gemini OpenAI-compatible adapter"));
    }

    #[test]
    fn gemini_semantic_compact_preserves_tail_of_large_active_request() {
        let active_request = format!("{}FINAL_ACTION_MARKER", "filler ".repeat(1_000));
        let request = serde_json::to_vec(&serde_json::json!({
            "input": [{
                "type": "message",
                "role": "user",
                "content": [{"type": "input_text", "text": active_request}]
            }]
        }))
        .unwrap();

        let summary =
            runtime_gemini_semantic_compact_continuation_summary("Keep working.", &request);

        assert!(summary.contains("[... middle truncated ...]"));
        assert!(summary.contains("FINAL_ACTION_MARKER"));
        assert!(summary.contains("Keep working."));
    }
}
