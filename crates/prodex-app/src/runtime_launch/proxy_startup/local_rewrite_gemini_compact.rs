use super::gemini_rewrite::RuntimeGeminiProviderAuth;
use super::local_rewrite::{
    RUNTIME_LOCAL_REWRITE_PROFILE, RuntimeLocalRewriteProxyShared,
    RuntimeLocalRewriteUpstreamResponse, RuntimeLocalRewriteUpstreamResult,
};
use super::local_rewrite_gemini::send_runtime_gemini_upstream_request;
use super::local_rewrite_response::runtime_local_rewrite_response_with_call_id;
use super::local_rewrite_response_spend::emit_runtime_gateway_response_spend_event_for_body;
use super::*;
use crate::{
    RUNTIME_PROXY_BUFFERED_RESPONSE_MAX_BYTES, RuntimeHeapTrimmedBufferedResponseParts,
    read_blocking_response_body_with_limit,
};
use anyhow::{Context, Result, bail};
#[cfg(test)]
use prodex_provider_core::GEMINI_PROVIDER_CORE_LOCAL_COMPACT_SUMMARY_PREFIX;
use prodex_provider_core::{
    ProviderEndpoint, gemini_provider_core_compact_response_body,
    gemini_provider_core_local_compact_summary,
    gemini_provider_core_semantic_compact_continuation_summary,
    gemini_provider_core_semantic_compact_request_body,
    gemini_provider_core_semantic_compact_summary,
};
use prodex_provider_spi::ProviderStreamMode;

#[cfg(test)]
const GEMINI_LOCAL_COMPACT_MAX_SUMMARY_BYTES: usize = 24 * 1024;

fn runtime_gemini_compact_error_log_value(_err: &anyhow::Error) -> String {
    "semantic_compact_failed".to_string()
}

pub(super) fn runtime_gemini_compact_response(
    request_id: u64,
    captured: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
    auth: &RuntimeGeminiProviderAuth,
) -> tiny_http::ResponseBox {
    let semantic = gemini_provider_core_semantic_compact_request_body(&captured.body)
        .map_err(anyhow::Error::msg)
        .and_then(|body| {
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
                ProviderEndpoint::Responses,
                ProviderStreamMode::Unary,
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
                RuntimeLocalRewriteUpstreamResponse::Streaming(_) => {
                    bail!("Gemini semantic compact unexpectedly returned a local stream")
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

    let (parts, provider_completed) = match semantic {
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
            (parts, true)
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
                        runtime_proxy_log_field(
                            "reason",
                            runtime_gemini_compact_error_log_value(&err),
                        ),
                    ],
                ),
            );
            (
                runtime_gemini_local_compact_response_parts(&captured.body),
                false,
            )
        }
    };
    if provider_completed {
        emit_runtime_gateway_response_spend_event_for_body(
            request_id,
            captured,
            shared,
            parts.status,
            0,
            parts.body.as_slice(),
        );
    }
    runtime_local_rewrite_response_with_call_id(parts, request_id, shared)
}

pub(super) fn runtime_gemini_semantic_compact_response_parts(
    status: u16,
    response: reqwest::blocking::Response,
    request_id: u64,
    compact_request_body: &[u8],
) -> Result<RuntimeHeapTrimmedBufferedResponseParts> {
    let body = read_blocking_response_body_with_limit(
        response,
        RUNTIME_PROXY_BUFFERED_RESPONSE_MAX_BYTES,
        "failed to read Gemini semantic compact response",
    )?;
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
    let summary = gemini_provider_core_semantic_compact_summary(value, request_id);
    if summary.is_empty() {
        bail!("Gemini semantic compact returned no summary text");
    }
    let summary =
        gemini_provider_core_semantic_compact_continuation_summary(&summary, compact_request_body);
    Ok(runtime_gemini_compact_response_parts(&summary))
}

pub(super) fn runtime_gemini_local_compact_response_parts(
    body: &[u8],
) -> RuntimeHeapTrimmedBufferedResponseParts {
    let summary = gemini_provider_core_local_compact_summary(body);
    runtime_gemini_compact_response_parts(&summary)
}

pub(super) fn runtime_gemini_compact_response_parts(
    summary: &str,
) -> RuntimeHeapTrimmedBufferedResponseParts {
    RuntimeHeapTrimmedBufferedResponseParts {
        status: 200,
        headers: vec![(
            "content-type".to_string(),
            b"application/json; charset=utf-8".to_vec(),
        )],
        body: gemini_provider_core_compact_response_body(summary).into(),
    }
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
        assert!(text.contains(GEMINI_PROVIDER_CORE_LOCAL_COMPACT_SUMMARY_PREFIX));
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
                <= GEMINI_PROVIDER_CORE_LOCAL_COMPACT_SUMMARY_PREFIX.len()
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

        let translated = gemini_provider_core_semantic_compact_request_body(&body).unwrap();
        let value: serde_json::Value = serde_json::from_slice(&translated).unwrap();
        let input = value["input"].as_array().unwrap();

        assert_eq!(value["stream"], false);
        assert_eq!(value["store"], false);
        assert_eq!(value["parallel_tool_calls"], false);
        assert_eq!(
            value["model"],
            prodex_provider_core::PRODEX_GEMINI_CHAT_COMPRESSION_MODEL
        );
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
        assert!(text.contains(GEMINI_PROVIDER_CORE_LOCAL_COMPACT_SUMMARY_PREFIX));
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
    fn gemini_compact_error_log_value_is_content_free() {
        let err = anyhow::anyhow!(
            "compact failed\nAuthorization: Bearer gemini-compact-token\napi_key=gemini-compact-key"
        )
        .context("Gemini semantic compact fallback");
        let message = runtime_gemini_compact_error_log_value(&err);

        assert_eq!(message, "semantic_compact_failed");
        assert!(!message.contains("gemini-compact-token"));
        assert!(!message.contains("gemini-compact-key"));
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
            gemini_provider_core_semantic_compact_continuation_summary("Keep working.", &request);

        assert!(summary.contains("[... middle truncated ...]"));
        assert!(summary.contains("FINAL_ACTION_MARKER"));
        assert!(summary.contains("Keep working."));
    }
}
