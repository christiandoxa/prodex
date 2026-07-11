use super::{
    GeminiProviderCoreLiveAudioFormat, GeminiProviderCorePrecommitDecision,
    GeminiProviderCorePrecommitProbe, gemini_provider_core_append_media_parts_to_last_user_content,
    gemini_provider_core_apply_gemini3_tool_declaration_overrides,
    gemini_provider_core_blocked_tool_call_item, gemini_provider_core_body_has_terminal_quota,
    gemini_provider_core_bool_str, gemini_provider_core_bool_value,
    gemini_provider_core_buffered_responses_value,
    gemini_provider_core_buffered_responses_value_with_fallback_ids,
    gemini_provider_core_chat_assistant_messages,
    gemini_provider_core_chat_assistant_tool_call_item, gemini_provider_core_chat_message_text,
    gemini_provider_core_citation_text, gemini_provider_core_collect_input_texts,
    gemini_provider_core_collect_media_parts, gemini_provider_core_collect_path_values,
    gemini_provider_core_collect_string_values, gemini_provider_core_content,
    gemini_provider_core_contextual_user_instruction_text,
    gemini_provider_core_conversation_requests_command_output_only,
    gemini_provider_core_custom_tool_input_from_arguments, gemini_provider_core_data_url_parts,
    gemini_provider_core_exact_output_generate_chunk, gemini_provider_core_exact_output_sse_stream,
    gemini_provider_core_forced_command_output, gemini_provider_core_function_call_part,
    gemini_provider_core_function_call_part_from_tool_call,
    gemini_provider_core_function_response_content_part,
    gemini_provider_core_function_response_from_tool_message,
    gemini_provider_core_function_response_part, gemini_provider_core_function_tools_from_chat,
    gemini_provider_core_gemini3_tool_description,
    gemini_provider_core_generate_content_body_value,
    gemini_provider_core_generate_content_request,
    gemini_provider_core_generate_content_request_map, gemini_provider_core_google_quota_message,
    gemini_provider_core_harden_contents, gemini_provider_core_harden_tool_call_thought_signatures,
    gemini_provider_core_image_url_value, gemini_provider_core_import_contents_from_value,
    gemini_provider_core_invalid_stream_retry_delay_ms,
    gemini_provider_core_is_contextual_user_fragment,
    gemini_provider_core_live_audio_config_from_value,
    gemini_provider_core_live_audio_rate_from_mime,
    gemini_provider_core_live_audio_stream_end_message,
    gemini_provider_core_live_binary_frame_error, gemini_provider_core_live_client_content_message,
    gemini_provider_core_live_conversation_item_deleted_event,
    gemini_provider_core_live_conversation_item_truncated_event,
    gemini_provider_core_live_decode_alaw, gemini_provider_core_live_decode_ulaw,
    gemini_provider_core_live_error_event, gemini_provider_core_live_function_call_done_event,
    gemini_provider_core_live_function_declaration,
    gemini_provider_core_live_input_audio_cleared_event,
    gemini_provider_core_live_input_audio_payload,
    gemini_provider_core_live_input_audio_transcription_completed_event,
    gemini_provider_core_live_input_audio_transcription_delta_event,
    gemini_provider_core_live_legacy_session_audio_config,
    gemini_provider_core_live_output_audio_delta_event,
    gemini_provider_core_live_output_audio_transcript_delta_event,
    gemini_provider_core_live_output_audio_transcript_done_event,
    gemini_provider_core_live_output_text_delta_event,
    gemini_provider_core_live_output_text_done_event,
    gemini_provider_core_live_provider_stream_error,
    gemini_provider_core_live_realtime_audio_message,
    gemini_provider_core_live_response_cancelled_event,
    gemini_provider_core_live_response_created_event,
    gemini_provider_core_live_response_done_event, gemini_provider_core_live_server_turn_complete,
    gemini_provider_core_live_session_audio_config,
    gemini_provider_core_live_session_updated_event, gemini_provider_core_live_setup_message,
    gemini_provider_core_live_tool_response_message, gemini_provider_core_live_transcript_delta,
    gemini_provider_core_live_transcription_text,
    gemini_provider_core_live_unsupported_event_error, gemini_provider_core_local_compact_summary,
    gemini_provider_core_local_context_text_part,
    gemini_provider_core_mask_tool_response_for_history,
    gemini_provider_core_media_part_from_content_object, gemini_provider_core_media_part_from_data,
    gemini_provider_core_media_part_from_uri_or_data_url, gemini_provider_core_mime_type_for_uri,
    gemini_provider_core_mime_type_is_text, gemini_provider_core_model_uses_gemini3_toolset,
    gemini_provider_core_native_request_body_with_project,
    gemini_provider_core_non_actionable_wait_or_poll_text,
    gemini_provider_core_normalize_tool_name, gemini_provider_core_normalized_error_body,
    gemini_provider_core_parse_command_specific_tool,
    gemini_provider_core_precommit_decision_for_data_lines, gemini_provider_core_request_body,
    gemini_provider_core_response_bindings_from_body,
    gemini_provider_core_response_id_from_responses_value,
    gemini_provider_core_response_retryable_quota,
    gemini_provider_core_response_terminal_without_history,
    gemini_provider_core_response_tool_call_added_item,
    gemini_provider_core_response_tool_call_item, gemini_provider_core_response_tool_call_raw_item,
    gemini_provider_core_retry_delay_ms, gemini_provider_core_retry_delay_ms_from_body,
    gemini_provider_core_runtime_responses_value,
    gemini_provider_core_semantic_compact_continuation_summary,
    gemini_provider_core_semantic_compact_request_body,
    gemini_provider_core_semantic_compact_summary, gemini_provider_core_session_checkpoint_value,
    gemini_provider_core_should_inline_rate_limit_retry,
    gemini_provider_core_should_rotate_after_quota_response, gemini_provider_core_simple_request,
    gemini_provider_core_simple_response, gemini_provider_core_skip_context_path_name,
    gemini_provider_core_stream_error, gemini_provider_core_structured_command_tool_response,
    gemini_provider_core_system_instruction_from_chat, gemini_provider_core_text_part,
    gemini_provider_core_thought_signature, gemini_provider_core_tool_call_command_text,
    gemini_provider_core_tool_call_ids_from_responses_value,
    gemini_provider_core_tool_intent_without_call, gemini_provider_core_tool_is_mutating,
    gemini_provider_core_tool_output_call_ids_from_request,
    gemini_provider_core_tool_output_preview, gemini_provider_core_tool_response_output_string,
    gemini_provider_core_tools_from_requests, gemini_provider_core_unsupported_tool_fallback_body,
    gemini_provider_core_unverified_success_claim,
    gemini_provider_core_web_search_call_from_grounding,
};
use crate::{
    ProviderEndpoint, ProviderId, ProviderTransformLoss, ProviderTransformResult,
    ProviderWireFormat,
};
use std::collections::{BTreeMap, HashMap};

#[test]
fn gemini_provider_core_simple_request_accepts_plain_text_and_context_blocks() {
    let body = serde_json::to_vec(&serde_json::json!({
        "model": "gemini-2.5-pro",
        "input": [
            {"role": "system", "content": "You are precise."},
            {"role": "user", "content": "# AGENTS.md instructions for /repo"},
            {"role": "user", "content": "Fix the test"}
        ]
    }))
    .unwrap();
    assert!(gemini_provider_core_simple_request(&body));
}

#[test]
fn gemini_provider_core_simple_request_rejects_tools_and_unsupported_assistant_history() {
    let tools = serde_json::to_vec(&serde_json::json!({
        "model": "gemini-2.5-pro",
        "input": "hello",
        "tools": [{"type":"function","function":{"name":"search"}}]
    }))
    .unwrap();
    assert!(!gemini_provider_core_simple_request(&tools));

    let assistant = serde_json::to_vec(&serde_json::json!({
        "model": "gemini-2.5-pro",
        "input": [
            {
                "role":"assistant",
                "content":"hi",
                "gemini_native_parts":[{"text":"native"}]
            }
        ]
    }))
    .unwrap();
    assert!(!gemini_provider_core_simple_request(&assistant));
}

#[test]
fn gemini_provider_core_simple_request_accepts_builtin_tools() {
    let body = serde_json::to_vec(&serde_json::json!({
        "model": "gemini-2.5-pro",
        "input": "hello",
        "tools": [
            {"type":"web_search_preview"},
            {"type":"code_interpreter"},
            {
                "type":"computer_use_preview",
                "environment":"ENVIRONMENT_BROWSER"
            },
            {"type":"web_fetch_preview"}
        ],
        "tool_choice": "auto"
    }))
    .unwrap();
    assert!(gemini_provider_core_simple_request(&body));
}

#[test]
fn gemini_provider_core_simple_request_accepts_text_format_and_cache_fields() {
    let body = serde_json::to_vec(&serde_json::json!({
        "model": "gemini-2.5-pro",
        "input": "hello",
        "cached_content": "cachedContents/abc123",
        "text": {
            "format": {
                "type": "json_object"
            }
        }
    }))
    .unwrap();
    assert!(gemini_provider_core_simple_request(&body));
}

#[test]
fn gemini_provider_core_simple_request_accepts_assistant_and_tool_history() {
    let body = serde_json::to_vec(&serde_json::json!({
        "model": "gemini-2.5-pro",
        "input": [
            {"role":"user","content":"find it"},
            {
                "role":"assistant",
                "content":"",
                "tool_calls":[
                    {
                        "id":"call_1",
                        "type":"function",
                        "function":{
                            "name":"grep",
                            "arguments":"{\"pattern\":\"x\"}"
                        }
                    }
                ]
            },
            {
                "role":"tool",
                "tool_call_id":"call_1",
                "content":"{\"match_count\":1}"
            }
        ]
    }))
    .unwrap();
    assert!(gemini_provider_core_simple_request(&body));
}

#[test]
fn gemini_provider_core_simple_request_accepts_message_content_arrays() {
    let body = serde_json::to_vec(&serde_json::json!({
            "model": "gemini-2.5-pro",
            "input": [
                {
                    "type": "message",
                    "role": "user",
                    "content": [
                        {"type": "input_text", "text": "# AGENTS.md instructions for /repo"},
                        {"type": "input_text", "text": "<environment_context><cwd>/repo</cwd></environment_context>"}
                    ]
                },
                {
                    "type": "message",
                    "role": "assistant",
                    "content": [{"type": "output_text", "text": ""}],
                    "tool_calls": [
                        {
                            "id":"call_1",
                            "type":"function",
                            "function":{
                                "name":"grep",
                                "arguments":"{\"pattern\":\"x\"}"
                            }
                        }
                    ]
                },
                {
                    "type": "message",
                    "role": "tool",
                    "tool_call_id": "call_1",
                    "content": [{"type": "output_text", "text": "{\"match_count\":1}"}]
                }
            ]
        }))
        .unwrap();
    assert!(gemini_provider_core_simple_request(&body));
}

#[test]
fn gemini_provider_core_request_body_preserves_project_and_generation_config() {
    let result = ProviderTransformResult {
        provider: ProviderId::Gemini,
        endpoint: ProviderEndpoint::Responses,
        from_format: ProviderWireFormat::OpenAiResponses,
        to_format: ProviderWireFormat::GeminiGenerateContent,
        body: Some(
            serde_json::to_vec(&serde_json::json!({
                "model": "gemini-2.5-pro",
                "request": {
                    "contents": [{"role":"user","parts":[{"text":"hello"}]}]
                }
            }))
            .unwrap(),
        ),
        headers: Default::default(),
        metadata: Default::default(),
        loss: ProviderTransformLoss::Lossless,
    };
    let translated = serde_json::to_vec(&serde_json::json!({
        "model": "gemini-2.5-pro",
        "project": "project-1",
        "request": {
            "contents": [{"role":"user","parts":[{"text":"hello"}]}],
            "generationConfig": {"includeThoughts": true, "thinkingBudget": 8192}
        }
    }))
    .unwrap();
    let merged = gemini_provider_core_request_body(&result, &translated).unwrap();
    let merged: serde_json::Value = serde_json::from_slice(&merged).unwrap();
    assert_eq!(merged["project"], "project-1");
    assert_eq!(
        merged["request"]["generationConfig"]["thinkingBudget"],
        8192
    );
}

#[test]
fn gemini_provider_core_simple_response_accepts_prompt_feedback_block() {
    let response = serde_json::json!({
        "responseId": "resp_blocked",
        "modelVersion": "gemini-2.5-pro",
        "promptFeedback": {"blockReason": "SAFETY"}
    });
    assert!(gemini_provider_core_simple_response(&response, |_, _| None));
}

#[test]
fn gemini_provider_core_simple_response_accepts_thought_plus_safe_function_call_candidate() {
    let response = serde_json::json!({
        "responseId": "resp_reason_call",
        "modelVersion": "gemini-2.5-pro",
        "candidates": [{
            "content": {"parts": [
                {"text": "internal summary", "thought": true},
                {
                    "thoughtSignature": "sig-response-1",
                    "functionCall": {
                        "id": "call_sqz_1",
                        "name": "mcp__prodex_sqz__compress",
                        "args": {"text": "large content"}
                    }
                }
            ]}
        }]
    });
    assert!(gemini_provider_core_simple_response(&response, |_, _| None));
}

#[test]
fn gemini_provider_core_simple_response_rejects_blocked_function_calls() {
    let response = serde_json::json!({
        "responseId": "resp_blocked_tool",
        "modelVersion": "gemini-2.5-pro",
        "candidates": [{
            "content": {"parts": [{
                "functionCall": {
                    "id": "call_shell_1",
                    "name": "shell",
                    "args": {"cmd": "rm -rf /"}
                }
            }]}
        }]
    });
    assert!(!gemini_provider_core_simple_response(
        &response,
        |name, _| { (name == "shell").then(|| "blocked".to_string()) }
    ));
}

#[test]
fn gemini_provider_core_custom_tool_input_extracts_fenced_apply_patch_block() {
    let arguments = serde_json::json!({
        "input": "Here is the patch:\n```patch\n*** Begin Patch\n*** Add File: note.txt\n+hello\n*** End Patch\n```\n"
    });

    assert_eq!(
        gemini_provider_core_custom_tool_input_from_arguments(&arguments.to_string()),
        "*** Begin Patch\n*** Add File: note.txt\n+hello\n*** End Patch"
    );
}

#[test]
fn gemini_provider_core_custom_tool_input_normalizes_gemini_replace_args() {
    let arguments = serde_json::json!({
        "file_path": "src/lib.rs",
        "old_string": "let value = 1;\n",
        "new_string": "let value = 2;\n"
    });

    assert_eq!(
        gemini_provider_core_custom_tool_input_from_arguments(&arguments.to_string()),
        "\
*** Begin Patch
*** Update File: src/lib.rs
@@
-let value = 1;
+let value = 2;
*** End Patch"
    );
}

#[test]
fn gemini_provider_core_citation_text_dedupes_sources() {
    let response = serde_json::json!({
        "candidates": [{
            "finishReason": "STOP",
            "citationMetadata": {
                "citations": [
                    {"title": "A", "uri": "https://a.example"},
                    {"title": "A", "uri": "https://a.example"}
                ]
            }
        }]
    });

    assert_eq!(
        gemini_provider_core_citation_text(&response).as_deref(),
        Some("Citations:\n(A) https://a.example")
    );
}

#[test]
fn gemini_provider_core_web_search_call_from_grounding_collects_queries_and_sources() {
    let response = serde_json::json!({
        "candidates": [{
            "groundingMetadata": {
                "webSearchQueries": ["prodex", "gemini"],
                "groundingChunks": [{
                    "web": {"title": "Prodex", "uri": "https://prodex.example"}
                }]
            }
        }]
    });

    let item = gemini_provider_core_web_search_call_from_grounding(&response, "resp_1").unwrap();
    assert_eq!(item["type"], "web_search_call");
    assert_eq!(item["action"]["type"], "search");
    assert_eq!(item["action"]["queries"][0], "prodex");
    assert_eq!(
        item["action"]["sources"][0]["url"],
        "https://prodex.example"
    );
}

#[test]
fn gemini_provider_core_response_tool_call_item_preserves_override_call_id() {
    let part = serde_json::json!({
        "thoughtSignature": "sig-1"
    });
    let function_call = serde_json::json!({
        "name": "mcp__prodex_sqz__compress",
        "args": {"text": "large content"}
    });

    let item = gemini_provider_core_response_tool_call_item(
        &part,
        &function_call,
        Some("call_override_1"),
        |_, _| None,
    );
    assert_eq!(item["type"], "function_call");
    assert_eq!(item["call_id"], "call_override_1");
    assert_eq!(item["namespace"], "mcp__prodex_sqz");
    assert_eq!(item["name"], "compress");
    assert_eq!(item["gemini_thought_signature"], "sig-1");
}

#[test]
fn gemini_provider_core_response_tool_call_item_emits_blocked_message() {
    let part = serde_json::json!({});
    let function_call = serde_json::json!({
        "name": "shell",
        "args": {"cmd": "rm -rf /"}
    });

    let item = gemini_provider_core_response_tool_call_item(
        &part,
        &function_call,
        Some("call_shell_1"),
        |name, _| (name == "shell").then(|| "blocked".to_string()),
    );
    assert_eq!(item["type"], "message");
    assert_eq!(item["content"][0]["text"], "blocked");
}

#[test]
fn gemini_provider_core_response_tool_call_added_item_keeps_function_call_shape() {
    let part = serde_json::json!({
        "thoughtSignature": "sig-1"
    });
    let function_call = serde_json::json!({
        "name": "mcp__prodex_sqz__compress",
        "args": {"text": "large content"}
    });

    let item = gemini_provider_core_response_tool_call_added_item(
        &part,
        &function_call,
        Some("call_override_1"),
    )
    .unwrap();
    assert_eq!(item["type"], "function_call");
    assert_eq!(item["call_id"], "call_override_1");
    assert_eq!(item["namespace"], "mcp__prodex_sqz");
    assert_eq!(item["name"], "compress");
    assert!(item.get("arguments").is_none());
    assert_eq!(item["gemini_thought_signature"], "sig-1");
}

#[test]
fn gemini_provider_core_response_tool_call_added_item_skips_tool_search_and_apply_patch() {
    let part = serde_json::json!({});
    let tool_search = serde_json::json!({
        "name": "tool_search",
        "args": {"query": "prodex"}
    });
    let apply_patch = serde_json::json!({
        "name": "apply_patch",
        "args": {"patch": "*** Begin Patch\n*** End Patch\n"}
    });

    assert!(
        gemini_provider_core_response_tool_call_added_item(&part, &tool_search, Some("call_1"))
            .is_none()
    );
    assert!(
        gemini_provider_core_response_tool_call_added_item(&part, &apply_patch, Some("call_2"))
            .is_none()
    );
}

#[test]
fn gemini_provider_core_response_tool_call_raw_item_keeps_raw_arguments_string() {
    let part = serde_json::json!({
        "thoughtSignature": "sig-raw-1"
    });
    let item = gemini_provider_core_response_tool_call_raw_item(
        &part,
        "mcp__prodex_sqz__compress",
        "{\"broken\":",
        Some("call_raw_1"),
    );
    assert_eq!(item["type"], "function_call");
    assert_eq!(item["call_id"], "call_raw_1");
    assert_eq!(item["namespace"], "mcp__prodex_sqz");
    assert_eq!(item["name"], "compress");
    assert_eq!(item["arguments"], "{\"broken\":");
    assert_eq!(item["gemini_thought_signature"], "sig-raw-1");
}

#[test]
fn gemini_provider_core_chat_assistant_tool_call_item_wraps_arguments_and_preserves_signature() {
    let part = serde_json::json!({
        "thoughtSignature": "sig-chat-1"
    });
    let function_call = serde_json::json!({
        "name": "shell",
        "args": {"cmd": "pwd"}
    });
    let item = gemini_provider_core_chat_assistant_tool_call_item(
        &part,
        &function_call,
        Some("call_chat_1"),
    );
    assert_eq!(item["id"], "call_chat_1");
    assert_eq!(item["type"], "function");
    assert_eq!(item["function"]["name"], "shell");
    assert_eq!(item["function"]["arguments"], "{\"cmd\":\"pwd\"}");
    assert_eq!(item["gemini_thought_signature"], "sig-chat-1");
}

#[test]
fn gemini_provider_core_chat_assistant_messages_builds_history_item() {
    let response = serde_json::json!({
        "candidates": [{
            "content": {
                "parts": [{
                    "thoughtSignature": "sig-fn-1",
                    "functionCall": {
                        "id": "call_google_1",
                        "name": "mcp__prodex_sqz__sqz_read_file",
                        "args": {"path": "README.md"}
                    }
                }]
            },
            "finishReason": "STOP"
        }]
    });
    let assistant = gemini_provider_core_chat_assistant_messages(&response, 12, |_, _| None);
    assert_eq!(assistant[0]["role"], "assistant");
    assert_eq!(assistant[0]["tool_calls"][0]["id"], "call_google_1");
    assert_eq!(
        assistant[0]["tool_calls"][0]["gemini_thought_signature"],
        "sig-fn-1"
    );
}

#[test]
fn gemini_provider_core_runtime_responses_value_preserves_runtime_shape() {
    let response = serde_json::json!({
        "candidates": [{
            "content": {
                "parts": [
                    {"text": "hidden", "thought": true},
                    {"text": "visible"},
                    {"functionCall": {"name": "demo", "args": {"x": 1}}}
                ]
            },
            "finishReason": "STOP"
        }],
        "usageMetadata": {
            "promptTokenCount": 1,
            "candidatesTokenCount": 2,
            "totalTokenCount": 3
        },
        "responseMetadata": {
            "prompt_token_details": [{"modality": "TEXT", "tokenCount": 1}]
        }
    });

    let value = gemini_provider_core_runtime_responses_value(
        &response,
        12,
        1234,
        "gemini-default",
        |_, _| None,
    );

    assert_eq!(value["id"], "resp_gemini_12");
    assert_eq!(value["created_at"], 1234);
    assert_eq!(value["model"], "gemini-default");
    assert_eq!(value["output"][0]["type"], "function_call");
    assert!(value.get("usage").is_some());
    assert!(value.get("metadata").is_some());
    assert_eq!(value["output"].as_array().unwrap().len(), 1);
}

#[test]
fn gemini_provider_core_builds_buffered_responses_value() {
    let response = serde_json::json!({
        "candidates": [{
            "content": {"parts": [{"text": "hello"}]},
            "finishReason": "STOP"
        }]
    });
    let value = gemini_provider_core_buffered_responses_value(&response, 99, None, |_, _| None);
    assert_eq!(value["id"], "resp_gemini_99");
    assert_eq!(value["output"][0]["content"][0]["text"], "hello");
    assert!(value.get("created_at").is_some());
    assert!(gemini_provider_core_response_terminal_without_history(
        &serde_json::json!({"status": "failed"})
    ));
    assert!(gemini_provider_core_response_terminal_without_history(
        &serde_json::json!({"error": {"message": "no"}})
    ));
    assert!(!gemini_provider_core_response_terminal_without_history(
        &value
    ));
}

#[test]
fn gemini_provider_core_bypasses_translated_placeholder_ids() {
    let response = serde_json::json!({
        "candidates": [{
            "content": {
                "parts": [{"functionCall": {"name": "shell", "args": {}}}]
            }
        }]
    });
    let translated = ProviderTransformResult::lossless(
        ProviderId::Gemini,
        ProviderEndpoint::Responses,
        ProviderWireFormat::GeminiGenerateContent,
        ProviderWireFormat::OpenAiResponses,
        serde_json::to_vec(&serde_json::json!({
            "id": "gemini_resp_prodex",
            "output": [{"type": "function_call", "call_id": "call_1"}]
        }))
        .unwrap(),
    );

    let value = gemini_provider_core_buffered_responses_value_with_fallback_ids(
        &response,
        Some(&translated),
        |_, _| None,
        || "resp_gemini_request-v7".to_string(),
        |_| "call_gemini_call-v7".to_string(),
    );

    assert_eq!(value["id"], "resp_gemini_request-v7");
    assert_eq!(value["output"][0]["call_id"], "call_gemini_call-v7");
}

#[test]
fn gemini_provider_core_unsupported_tool_fallback_body_removes_first_matching_tool() {
    let body = serde_json::to_vec(&serde_json::json!({
        "request": {
            "tools": [
                {"googleSearch": {}},
                {"codeExecution": {}},
                {"functionDeclarations": [{"name": "keep"}]}
            ]
        }
    }))
    .unwrap();

    let (tool_name, stripped) = gemini_provider_core_unsupported_tool_fallback_body(&body).unwrap();
    let stripped: serde_json::Value = serde_json::from_slice(&stripped).unwrap();

    assert_eq!(tool_name, "codeExecution");
    assert_eq!(stripped["request"]["tools"].as_array().unwrap().len(), 2);
    assert_eq!(
        stripped["request"]["tools"][0],
        serde_json::json!({"googleSearch": {}})
    );
}

#[test]
fn gemini_provider_core_media_part_from_data_handles_data_urls_and_raw_data() {
    let data_url =
        gemini_provider_core_media_part_from_data("data:image/png;base64,abc123", None).unwrap();
    assert_eq!(data_url["inlineData"]["mimeType"], "image/png");
    assert_eq!(data_url["inlineData"]["data"], "abc123");

    let raw =
        gemini_provider_core_media_part_from_data(" raw-base64 ", Some("image/jpeg")).unwrap();
    assert_eq!(raw["inlineData"]["mimeType"], "image/jpeg");
    assert_eq!(raw["inlineData"]["data"], "raw-base64");
    assert!(gemini_provider_core_media_part_from_data("   ", None).is_none());
}

#[test]
fn gemini_provider_core_media_part_from_content_object_matches_runtime_shape() {
    let image = serde_json::json!({
        "type": "image_url",
        "image_url": {"url": "https://x.test/a.png"}
    });
    let image =
        gemini_provider_core_media_part_from_content_object(image.as_object().unwrap(), |_, _| {
            None
        })
        .unwrap();
    assert_eq!(image["fileData"]["mimeType"], "image/png");

    let file = serde_json::json!({
        "type": "input_file",
        "mimeType": "application/pdf",
        "fileData": "abc123"
    });
    let file =
        gemini_provider_core_media_part_from_content_object(file.as_object().unwrap(), |_, _| None)
            .unwrap();
    assert_eq!(file["inlineData"]["mimeType"], "application/pdf");
    assert_eq!(file["inlineData"]["data"], "abc123");

    let path = serde_json::json!({
        "type": "media",
        "media_type": "text/plain",
        "path": "/tmp/note.txt"
    });
    let path = gemini_provider_core_media_part_from_content_object(
        path.as_object().unwrap(),
        |path, mime| Some(serde_json::json!({"localPath": path, "mimeType": mime})),
    )
    .unwrap();
    assert_eq!(path["localPath"], "/tmp/note.txt");
    assert_eq!(path["mimeType"], "text/plain");
}

#[test]
fn gemini_provider_core_collects_and_appends_media_parts() {
    let input = serde_json::json!({
        "content": [
            {"type": "input_image", "image_url": "https://x.test/a.webp"},
            {"type": "input_file", "path": "/tmp/a.txt", "mime_type": "text/plain"}
        ]
    });
    let mut parts = Vec::new();
    gemini_provider_core_collect_media_parts(&input, &mut parts, &mut |path, mime| {
        Some(serde_json::json!({"localPath": path, "mimeType": mime}))
    });
    assert_eq!(parts.len(), 2);
    assert_eq!(parts[0]["fileData"]["mimeType"], "image/webp");
    assert_eq!(parts[1]["localPath"], "/tmp/a.txt");

    let mut contents = vec![serde_json::json!({
        "role": "user",
        "parts": [{"text": "hello"}]
    })];
    gemini_provider_core_append_media_parts_to_last_user_content(&mut contents, parts);
    assert_eq!(contents.len(), 1);
    assert_eq!(contents[0]["parts"].as_array().unwrap().len(), 3);
}

#[test]
fn gemini_provider_core_request_content_helpers_match_runtime_behavior() {
    let message = serde_json::json!({
        "content": [
            {"text": "hello"},
            {"content": "world"},
            {"ignored": true}
        ]
    });
    assert_eq!(
        gemini_provider_core_chat_message_text(&message).as_deref(),
        Some("hello\nworld")
    );
    assert_eq!(
        gemini_provider_core_chat_message_text(&serde_json::json!({"text": "ignored"})),
        None
    );
    assert_eq!(gemini_provider_core_text_part("hello")["text"], "hello");
    assert_eq!(
        gemini_provider_core_content("user", vec![gemini_provider_core_text_part("hello")])["parts"]
            [0]["text"],
        "hello"
    );

    assert_eq!(
        gemini_provider_core_image_url_value(&serde_json::json!({
            "image_url": "https://example.test/image.png"
        })),
        Some("https://example.test/image.png")
    );
    assert_eq!(
        gemini_provider_core_image_url_value(&serde_json::json!("data:image/png;base64,abc")),
        Some("data:image/png;base64,abc")
    );

    assert_eq!(
        gemini_provider_core_function_call_part(" call_1 ", "lookup", serde_json::json!({}))["id"],
        " call_1 "
    );
    assert!(
        gemini_provider_core_function_call_part(" ", "lookup", serde_json::json!({}))
            .get("id")
            .is_none()
    );

    let mut tool_names = BTreeMap::new();
    let tool_call = gemini_provider_core_function_call_part_from_tool_call(
        &serde_json::json!({
            "id": "call_3",
            "function": {
                "name": "lookup",
                "arguments": "{\"q\":\"rust\"}",
                "gemini_thought_signature": "sig"
            }
        }),
        &mut tool_names,
    )
    .unwrap();
    assert_eq!(tool_call["functionCall"]["name"], "lookup");
    assert_eq!(tool_call["functionCall"]["args"]["q"], "rust");
    assert_eq!(tool_call["thoughtSignature"], "sig");
    assert_eq!(tool_names.get("call_3").map(String::as_str), Some("lookup"));

    assert_eq!(
        gemini_provider_core_function_response_part(
            "call_2",
            "lookup",
            serde_json::json!({"ok": true})
        )["id"],
        "call_2"
    );
    assert_eq!(
        gemini_provider_core_function_response_content_part(
            gemini_provider_core_function_response_part(
                "call_2",
                "lookup",
                serde_json::json!({"ok": true})
            )
        )["functionResponse"]["name"],
        "lookup"
    );

    let mut tool_names = BTreeMap::new();
    tool_names.insert("call_3".to_string(), "lookup".to_string());
    let response = gemini_provider_core_function_response_from_tool_message(
        &serde_json::json!({
            "role": "tool",
            "tool_call_id": "call_3",
            "content": "{\"answer\":42}"
        }),
        &tool_names,
        |_, _, response| response,
    );
    assert_eq!(response["name"], "lookup");
    assert_eq!(response["id"], "call_3");
    assert_eq!(response["response"]["answer"], 42);
}

#[test]
fn gemini_provider_core_builds_runtime_system_instruction() {
    let chat = serde_json::json!({
        "messages": [
            {"role": "system", "content": "System"},
            {"role": "user", "content": "# AGENTS.md instructions for /repo\nKeep English."}
        ]
    });
    let value = gemini_provider_core_system_instruction_from_chat(
        &chat,
        &serde_json::json!({}),
        "Parity",
        "Tools",
        Some("Memory"),
        Some("Policy"),
    )
    .unwrap();
    let text = value["parts"][0]["text"].as_str().unwrap();
    assert!(text.contains("System"));
    assert!(text.contains("Keep English."));
    assert!(text.contains("Parity"));
    assert!(text.contains("Tools"));
    assert!(text.contains("# Gemini CLI Memory Compatibility\nMemory"));
    assert!(text.contains("# Gemini CLI Policy Compatibility\nPolicy"));

    let compact = gemini_provider_core_system_instruction_from_chat(
        &chat,
        &serde_json::json!({"prodex_gemini_compaction": true}),
        "Parity",
        "Tools",
        Some("Memory"),
        Some("Policy"),
    )
    .unwrap();
    let compact_text = compact["parts"][0]["text"].as_str().unwrap();
    assert!(compact_text.contains("System"));
    assert!(!compact_text.contains("Parity"));
    assert!(!compact_text.contains("Memory"));
}

#[test]
fn gemini_provider_core_imports_session_contents_from_values_and_text() {
    let contents = gemini_provider_core_import_contents_from_value(
        &serde_json::json!({
            "history": [
                {"role": "assistant", "content": "Model text"},
                {"role": "user", "parts": [{"text": "User part"}]}
            ]
        }),
        20,
    );
    assert_eq!(contents[0]["role"], "model");
    assert_eq!(contents[0]["parts"][0]["text"], "Model text");
    assert_eq!(contents[1]["role"], "user");
    assert_eq!(contents[1]["parts"][0]["text"], "User part");

    let jsonl = "{\"role\":\"assistant\",\"text\":\"A\"}\n{\"content\":\"B\"}";
    let contents = gemini_provider_core_import_contents_from_value(&serde_json::json!(jsonl), 20);
    assert_eq!(contents.len(), 2);
    assert_eq!(contents[0]["role"], "model");
    assert_eq!(contents[1]["parts"][0]["text"], "B");

    let fallback = gemini_provider_core_import_contents_from_value(&serde_json::json!("abcdef"), 3);
    assert_eq!(
        fallback[0]["parts"][0]["text"],
        "Imported Gemini session/checkpoint context:\nabc"
    );
}

#[test]
fn gemini_provider_core_builds_session_checkpoint_value() {
    let mut request = serde_json::Map::new();
    request.insert("contents".to_string(), serde_json::json!([]));
    let checkpoint = gemini_provider_core_session_checkpoint_value(&request);
    assert_eq!(checkpoint["version"], 1);
    assert_eq!(checkpoint["format"], "gemini-generate-content");
    assert_eq!(checkpoint["source"], "prodex-gemini-bridge");
    assert!(checkpoint["request"]["contents"].is_array());
}

#[test]
fn gemini_provider_core_builds_generate_content_request_and_body() {
    let request = gemini_provider_core_generate_content_request_map(
        &serde_json::json!({
            "safety_settings": [{"category": "HARM_CATEGORY_DANGEROUS_CONTENT"}],
            "cachedContent": "cachedContents/1",
            "labels": {"app": "prodex"}
        }),
        Some(serde_json::json!({"parts": [{"text": "system"}]})),
        vec![serde_json::json!({"role": "user", "parts": [{"text": "hi"}]})],
        Some(serde_json::json!([{"googleSearch": {}}])),
        Some(serde_json::json!({"functionCallingConfig": {"mode": "ANY"}})),
        serde_json::json!({"temperature": 0.2}),
    );
    assert_eq!(request["systemInstruction"]["parts"][0]["text"], "system");
    assert_eq!(request["contents"].as_array().unwrap().len(), 1);
    assert!(request["tools"].is_array());
    assert_eq!(request["generationConfig"]["temperature"], 0.2);
    assert_eq!(request["cachedContent"], "cachedContents/1");
    assert_eq!(request["labels"]["app"], "prodex");

    let body =
        gemini_provider_core_generate_content_body_value("gemini", Some("project"), true, &request);
    assert_eq!(body["model"], "gemini");
    assert_eq!(body["project"], "project");
    assert_eq!(body["request"]["contents"].as_array().unwrap().len(), 1);
    let direct = gemini_provider_core_generate_content_body_value("gemini", None, false, &request);
    assert!(direct.get("request").is_none());
    assert!(direct["contents"].is_array());
}

#[test]
fn gemini_provider_core_builds_generate_content_payload() {
    let result = gemini_provider_core_generate_content_request(
        &serde_json::json!({"safety_settings": [{"category": "HARASSMENT"}]}),
        &serde_json::json!({
            "model": "gemini-2.5-pro",
            "stream": true,
            "messages": [{"role":"user","parts":[{"text":"hello"}]}]
        }),
        "fallback-model",
        Some("proj"),
        true,
        Some(128),
        Some(serde_json::json!({"parts":[{"text":"system"}]})),
        Some(serde_json::json!([])),
        Some(serde_json::json!({"toolCallingConfig":{"mode":"AUTO"}})),
    );
    assert_eq!(result.model, "gemini-2.5-pro");
    assert!(result.stream);
    assert_eq!(result.body["model"], "gemini-2.5-pro");
    assert_eq!(result.body["project"], "proj");
    assert_eq!(
        result.request["systemInstruction"]["parts"][0]["text"],
        "system"
    );
    assert_eq!(
        result.request["toolConfig"]["toolCallingConfig"]["mode"],
        "AUTO"
    );
    assert!(result.body["request"]["contents"][0]["role"] == "user");
    assert_eq!(
        result.body["request"]["safetySettings"][0]["category"],
        "HARASSMENT"
    );
}

#[test]
fn gemini_provider_core_stamps_native_project_fields() {
    let body = serde_json::to_vec(&serde_json::json!({
        "project": "old",
        "projectId": "old",
        "cloudaicompanionProject": "old",
        "metadata": {"duetProject": "old"},
        "request": {
            "project": "old",
            "metadata": {"duetProject": "old"}
        }
    }))
    .unwrap();
    let rewritten =
        gemini_provider_core_native_request_body_with_project(&body, Some("selected")).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&rewritten).unwrap();
    assert_eq!(value["project"], "selected");
    assert_eq!(value["projectId"], "selected");
    assert_eq!(value["cloudaicompanionProject"], "selected");
    assert_eq!(value["metadata"]["duetProject"], "selected");
    assert_eq!(value["request"]["project"], "selected");
    assert_eq!(value["request"]["metadata"]["duetProject"], "selected");
    assert_eq!(
        gemini_provider_core_native_request_body_with_project(b"not-json", Some("selected"))
            .unwrap(),
        b"not-json"
    );
}

#[test]
fn gemini_provider_core_media_part_from_uri_detects_mime_type() {
    let file =
        gemini_provider_core_media_part_from_uri_or_data_url("https://x.test/a.PDF?dl=1", None)
            .unwrap();
    assert_eq!(file["fileData"]["fileUri"], "https://x.test/a.PDF?dl=1");
    assert_eq!(file["fileData"]["mimeType"], "application/pdf");

    let inline =
        gemini_provider_core_media_part_from_uri_or_data_url("data:;base64,abc123", None).unwrap();
    assert_eq!(inline["inlineData"]["mimeType"], "application/octet-stream");
    assert_eq!(inline["inlineData"]["data"], "abc123");
    assert_eq!(
        gemini_provider_core_mime_type_for_uri("file.unknown#anchor"),
        "application/octet-stream"
    );
    assert_eq!(
        gemini_provider_core_data_url_parts("data:text/plain;BASE64,abc"),
        Some(("text/plain", "abc"))
    );
    assert!(gemini_provider_core_mime_type_is_text("text/markdown"));
    assert!(gemini_provider_core_mime_type_is_text("application/json"));
    assert!(!gemini_provider_core_mime_type_is_text("image/png"));
    let mut values = Vec::new();
    gemini_provider_core_collect_string_values(
        Some(&serde_json::json!(["one", ["two", 3], null])),
        &mut values,
    );
    assert_eq!(values, vec!["one".to_string(), "two".to_string()]);
    let mut paths = Vec::new();
    gemini_provider_core_collect_path_values(
        Some(&serde_json::json!([" ./one.txt ", ["two.md", ""], 3])),
        &mut paths,
    );
    assert_eq!(
        paths,
        vec![std::path::PathBuf::from("./one.txt"), "two.md".into()]
    );
    let mut texts = Vec::new();
    gemini_provider_core_collect_input_texts(
        Some(&serde_json::json!([
            "plain",
            {"text": "text field"},
            {"content": [{"content": "nested content"}]},
            7
        ])),
        &mut texts,
    );
    assert_eq!(
        texts,
        vec!["plain", "text field", "nested content", "nested content"]
    );
    assert_eq!(gemini_provider_core_bool_str(" yes "), Some(true));
    assert_eq!(
        gemini_provider_core_bool_value(&serde_json::json!(0)),
        Some(false)
    );
    assert_eq!(
        gemini_provider_core_parse_command_specific_tool("shell(cargo test)"),
        Some(("shell".to_string(), "cargo test".to_string()))
    );
    assert_eq!(
        gemini_provider_core_parse_command_specific_tool("shell()"),
        None
    );
    assert!(gemini_provider_core_skip_context_path_name("node_modules"));
    assert!(!gemini_provider_core_skip_context_path_name("src"));
    assert_eq!(
        gemini_provider_core_local_context_text_part("src/lib.rs", "hello")["text"],
        "Content from @src/lib.rs:\nhello"
    );
}

#[test]
fn gemini_provider_core_contextual_user_instruction_detects_codex_context() {
    let message = serde_json::json!({
        "role": "user",
        "content": [{
            "type": "input_text",
            "text": "# AGENTS.md instructions for /repo\n\n<environment_context>\n  <cwd>/repo</cwd>\n</environment_context>"
        }]
    });
    let text = gemini_provider_core_contextual_user_instruction_text(&message).unwrap();
    assert!(text.contains("# AGENTS.md instructions for /repo"));
    assert!(text.contains("<environment_context>"));
    assert!(gemini_provider_core_is_contextual_user_fragment(
        "  <permissions instructions>"
    ));
    assert!(
        gemini_provider_core_contextual_user_instruction_text(
            &serde_json::json!({"role": "user", "content": "Run the update"})
        )
        .is_none()
    );
}

#[test]
fn gemini_provider_core_applies_gemini3_tool_declaration_overrides() {
    let mut declarations = vec![
        serde_json::json!({
            "name": "apply_patch",
            "parameters": {
                "type": "object",
                "properties": {
                    "input": {"type": "string"}
                }
            }
        }),
        serde_json::json!({
            "name": "mcp__repo__grep",
            "parameters": {
                "type": "object",
                "properties": {
                    "pattern": {"type": "string"}
                }
            }
        }),
    ];

    gemini_provider_core_apply_gemini3_tool_declaration_overrides("auto", &mut declarations);

    assert!(
        declarations[0]["description"]
            .as_str()
            .unwrap()
            .contains("Add File")
    );
    assert!(
        declarations[0]["parameters"]["properties"]["input"]["description"]
            .as_str()
            .unwrap()
            .contains("prefix every new file content line")
    );
    assert!(
        declarations[1]["parameters"]["properties"]["pattern"]["description"]
            .as_str()
            .unwrap()
            .contains("Literal or regex")
    );
    assert!(gemini_provider_core_model_uses_gemini3_toolset(
        "auto-gemini-3"
    ));
    assert!(
        gemini_provider_core_gemini3_tool_description("tools.run-shell-command")
            .unwrap()
            .contains("non-interactive")
    );

    let tools = gemini_provider_core_function_tools_from_chat(
            &serde_json::json!({
                "tools": [
                    {"type": "function", "function": {"name": "apply_patch", "parameters": {"type": "object"}}},
                    {"type": "function", "function": {"name": "read_file", "parameters": {"type": "object"}}}
                ]
            }),
            "auto",
            |declarations| declarations.retain(|declaration| declaration["name"] == "read_file"),
        )
        .unwrap();
    assert_eq!(
        tools[0]["functionDeclarations"].as_array().unwrap().len(),
        1
    );
    assert_eq!(tools[0]["functionDeclarations"][0]["name"], "read_file");

    let all_tools = gemini_provider_core_tools_from_requests(
            &serde_json::json!({
                "tools": [{"type": "web_search_preview"}]
            }),
            &serde_json::json!({
                "tools": [{"type": "function", "function": {"name": "read_file", "parameters": {"type": "object"}}}]
            }),
            "auto",
            |_| {},
        )
        .unwrap();
    assert_eq!(all_tools.as_array().unwrap().len(), 2);
}

#[test]
fn gemini_provider_core_detects_sse_guardrail_text() {
    assert_eq!(
        gemini_provider_core_tool_intent_without_call(
            "Now, I'll use `sqz_grep` to find the request builder."
        ),
        Some("sqz_grep")
    );
    assert_eq!(
        gemini_provider_core_non_actionable_wait_or_poll_text("The process is still running."),
        Some("still running")
    );
    assert_eq!(
        gemini_provider_core_tool_intent_without_call(
            "I will run through the plan without naming a tool."
        ),
        None
    );
}

#[test]
fn gemini_provider_core_detects_unverified_success_after_tool_error() {
    let messages = vec![
        serde_json::json!({"role": "user", "content": "update optional tools"}),
        serde_json::json!({
            "role": "tool",
            "content": "Process exited with code 2\nerror: failed"
        }),
    ];
    assert!(gemini_provider_core_unverified_success_claim(
        "Blocker/unresolved: none. Everything is complete.",
        &messages
    ));

    let verified = vec![
        serde_json::json!({"role": "user", "content": "check version"}),
        serde_json::json!({"role": "tool", "content": "prodex 1.2.3"}),
    ];
    assert!(!gemini_provider_core_unverified_success_claim(
        "latest version is installed",
        &verified
    ));
}

#[test]
fn gemini_provider_core_forces_exact_command_output_from_tool_message() {
    let messages = vec![
        serde_json::json!({
            "role": "user",
            "content": "Run printf forced-ok and answer with only the command output."
        }),
        serde_json::json!({
            "role": "assistant",
            "tool_calls": [{
                "function": {
                    "arguments": "{\"cmd\":\"printf forced-ok\"}"
                }
            }]
        }),
        serde_json::json!({
            "role": "tool",
            "content": "Process exited with code 0\nOutput:\nforced-ok\n"
        }),
    ];

    assert!(gemini_provider_core_conversation_requests_command_output_only(&messages));
    assert_eq!(
        gemini_provider_core_forced_command_output(&messages),
        Some("forced-ok".to_string())
    );
    let chunk = gemini_provider_core_exact_output_generate_chunk(7, "gemini-test", "forced-ok");
    assert_eq!(chunk["responseId"], "resp_gemini_exact_7");
    assert_eq!(chunk["modelVersion"], "gemini-test");
    assert_eq!(
        chunk["candidates"][0]["content"]["parts"][0]["text"],
        "forced-ok"
    );
    assert!(
        gemini_provider_core_exact_output_sse_stream(7, "gemini-test", "forced-ok")
            .contains("data: [DONE]")
    );
}

#[test]
fn gemini_provider_core_requires_matching_exact_output_command() {
    let messages = vec![
        serde_json::json!({
            "role": "user",
            "content": "Update demo.\nAfter updating, run exactly: ./bin/demo --version\nAnswer with only the command output."
        }),
        serde_json::json!({
            "role": "assistant",
            "tool_calls": [{
                "function": {
                    "arguments": "{\"cmd\":\"cat README.md\"}"
                }
            }]
        }),
        serde_json::json!({
            "role": "tool",
            "content": "Process exited with code 0\nOutput:\ndemo 1.2.3\n"
        }),
    ];

    assert_eq!(gemini_provider_core_forced_command_output(&messages), None);
}

#[test]
fn gemini_provider_core_builds_blocked_tool_call_item() {
    let item = gemini_provider_core_blocked_tool_call_item("blocked by policy");
    assert_eq!(item["type"], "message");
    assert_eq!(item["role"], "assistant");
    assert_eq!(item["content"][0]["type"], "output_text");
    assert_eq!(item["content"][0]["text"], "blocked by policy");
}

#[test]
fn gemini_provider_core_extracts_stream_error() {
    let value = serde_json::json!({
        "error": {
            "status": "RESOURCE_EXHAUSTED",
            "message": "quota hit"
        }
    });
    assert_eq!(
        gemini_provider_core_stream_error(&value),
        Some(("RESOURCE_EXHAUSTED".to_string(), "quota hit".to_string()))
    );

    let fallback = serde_json::json!({"error": {}});
    assert_eq!(
        gemini_provider_core_stream_error(&fallback),
        Some((
            "provider_stream_error".to_string(),
            "Gemini stream returned an embedded provider error".to_string()
        ))
    );
}

#[test]
fn gemini_provider_core_structures_running_command_output() {
    let output = "Chunk ID: abc123\nProcess running with session ID 48274\nOutput:\nCloning into '/tmp/gemini-cli'...\n";
    let response =
        gemini_provider_core_structured_command_tool_response("exec_command", output).unwrap();

    assert_eq!(response["output"], output);
    assert_eq!(response["status"], "running");
    assert_eq!(response["codex_tool_status"], "running");
    assert_eq!(response["running_session_id"], 48274_u64);
    assert!(
        response["next_required_action"]
            .as_str()
            .unwrap()
            .contains("write_stdin")
    );
}

#[test]
fn gemini_provider_core_structures_failed_command_output() {
    let missing = "Process exited with code 2\nOutput:\nls: cannot access '/tmp/refs/gemini-cli': No such file or directory\n";
    let response =
        gemini_provider_core_structured_command_tool_response("tools.shell", missing).unwrap();
    assert_eq!(response["status"], "failed");
    assert_eq!(response["failure_kind"], "missing_path");
    assert_eq!(response["affected_path"], "/tmp/refs/gemini-cli");
    assert_eq!(response["path_verified"], false);

    let clone = "Process exited with code 2\nOutput:\nCloning into '/tmp/refs/gemini-cli'...\nerror: The requested URL returned error: 404\n";
    let response =
        gemini_provider_core_structured_command_tool_response("mcp__exec_command", clone).unwrap();
    assert_eq!(response["failure_kind"], "clone_or_network");
    assert_eq!(response["affected_path"], "/tmp/refs/gemini-cli");
}

#[test]
fn gemini_provider_core_ignores_non_command_output() {
    assert!(
        gemini_provider_core_structured_command_tool_response(
            "read_file",
            "Process exited with code 0"
        )
        .is_none()
    );
    assert!(gemini_provider_core_structured_command_tool_response("shell", "").is_none());
}

#[test]
fn gemini_provider_core_harden_contents_repairs_tool_history() {
    let mut contents = vec![
        serde_json::json!({
            "role": "model",
            "parts": [{
                "functionCall": {"id": "call_missing", "name": "shell", "args": {"cmd": "pwd"}}
            }]
        }),
        serde_json::json!({
            "role": "user",
            "parts": [{
                "functionResponse": {
                    "id": "call_orphan",
                    "name": "grep",
                    "response": {"output": "orphan"}
                }
            }]
        }),
    ];

    gemini_provider_core_harden_contents(&mut contents);

    assert_eq!(contents[0]["role"], "user");
    assert_eq!(contents[1]["role"], "model");
    assert_eq!(contents[2]["role"], "user");
    assert_eq!(contents.len(), 3);
    assert_eq!(contents[1]["parts"][1]["functionCall"]["id"], "call_orphan");
    assert_eq!(
        contents[1]["parts"][1]["thoughtSignature"],
        "skip_thought_signature_validator"
    );
    assert_eq!(
        contents[2]["parts"][0]["functionResponse"]["id"],
        "call_missing"
    );
    assert_eq!(
        contents[2]["parts"][1]["functionResponse"]["id"],
        "call_orphan"
    );
}

#[test]
fn gemini_provider_core_harden_contents_orders_parallel_tool_responses() {
    let mut contents = vec![
        serde_json::json!({
            "role": "model",
            "parts": [
                {"functionCall": {"id": "call_first", "name": "shell", "args": {}}},
                {"functionCall": {"id": "call_second", "name": "shell", "args": {}}}
            ]
        }),
        serde_json::json!({
            "role": "user",
            "parts": [
                {"functionResponse": {"id": "call_second", "name": "shell", "response": {}}},
                {"functionResponse": {"id": "call_first", "name": "shell", "response": {}}},
                {"text": "done"}
            ]
        }),
    ];

    gemini_provider_core_harden_contents(&mut contents);

    let parts = contents[2]["parts"].as_array().unwrap();
    assert_eq!(parts[0]["functionResponse"]["id"], "call_first");
    assert_eq!(parts[1]["functionResponse"]["id"], "call_second");
    assert_eq!(parts[2]["text"], "done");
}

#[test]
fn gemini_provider_core_hardens_tool_call_thought_signatures_for_gemini3() {
    let mut body = serde_json::to_vec(&serde_json::json!({
        "request": {
            "contents": [{
                "role": "model",
                "parts": [
                    {"functionCall": {"name": "shell", "args": {"cmd": "pwd"}}},
                    {
                        "functionCall": {"name": "grep", "args": {"pattern": "x"}},
                        "thoughtSignature": "sig-keep"
                    }
                ]
            }]
        }
    }))
    .unwrap();

    let injected = gemini_provider_core_harden_tool_call_thought_signatures(
        &mut body,
        "gemini-3.1-pro-preview",
    )
    .unwrap();
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(injected, 1);
    assert_eq!(
        value["request"]["contents"][0]["parts"][0]["thoughtSignature"],
        "skip_thought_signature_validator"
    );
    assert_eq!(
        value["request"]["contents"][0]["parts"][1]["thoughtSignature"],
        "sig-keep"
    );
    assert_eq!(
        gemini_provider_core_thought_signature(&value["request"]["contents"][0]["parts"][1]),
        Some("sig-keep".to_string())
    );
}

#[test]
fn gemini_provider_core_skips_tool_call_thought_signatures_for_gemini2() {
    let mut body = serde_json::to_vec(&serde_json::json!({
        "contents": [{
            "role": "model",
            "parts": [{"functionCall": {"name": "shell", "args": {}}}]
        }]
    }))
    .unwrap();

    let injected =
        gemini_provider_core_harden_tool_call_thought_signatures(&mut body, "gemini-2.5-pro")
            .unwrap();
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(injected, 0);
    assert!(
        value["contents"][0]["parts"][0]
            .get("thoughtSignature")
            .is_none()
    );
}

#[test]
fn gemini_provider_core_masks_large_tool_response_for_history() {
    let response = serde_json::json!({
        "output": "abcdefghijklmnopqrstuvwxyz",
        "status": "ok"
    });

    let masked = gemini_provider_core_mask_tool_response_for_history(
        response,
        8,
        10,
        Some("/tmp/full-output.txt"),
    );

    assert_eq!(masked["_prodex_masked"], true);
    assert_eq!(masked["status"], "ok");
    let output = masked["output"].as_str().unwrap();
    assert!(output.contains("[tool_output_masked]"));
    assert!(output.contains("26 chars / 1 lines"));
    assert!(output.contains("Full output saved to: /tmp/full-output.txt"));
    assert!(output.contains("abcde\n...\nvwxyz"));
}

#[test]
fn gemini_provider_core_tool_output_preview_and_extracts_content() {
    let response = serde_json::json!({"content": "hello"});
    assert_eq!(
        gemini_provider_core_tool_response_output_string(&response),
        "hello"
    );
    assert_eq!(
        gemini_provider_core_tool_output_preview("abcdefghij", 4),
        "ab\n...\nij"
    );
    assert_eq!(
        gemini_provider_core_mask_tool_response_for_history(
            serde_json::json!({"output": "short"}),
            10,
            4,
            None
        )["output"],
        "short"
    );
}

#[test]
fn gemini_provider_core_detects_google_quota_and_retry_delay() {
    let quota_body = serde_json::to_vec(&serde_json::json!({
        "error": {
            "code": 429,
            "message": "Quota exceeded for quota metric.",
            "status": "RESOURCE_EXHAUSTED"
        }
    }))
    .unwrap();
    assert!(gemini_provider_core_response_retryable_quota(429));
    assert_eq!(
        gemini_provider_core_google_quota_message(&quota_body).as_deref(),
        Some("Quota exceeded for quota metric.")
    );

    let retry_body = serde_json::to_vec(&serde_json::json!({
        "error": {
            "code": 429,
            "message": "Resource exhausted.",
            "details": [{
                "@type": "type.googleapis.com/google.rpc.RetryInfo",
                "retryDelay": "34.074824224s"
            }]
        }
    }))
    .unwrap();
    assert_eq!(
        gemini_provider_core_retry_delay_ms_from_body(&retry_body),
        Some(34_075)
    );
    assert_eq!(
        gemini_provider_core_retry_delay_ms(None, b"", 0),
        Some(5_000)
    );
    assert_eq!(
        gemini_provider_core_retry_delay_ms(Some("99"), b"", 0),
        Some(99_000)
    );
    assert_eq!(
        gemini_provider_core_retry_delay_ms(Some("999"), b"", 0),
        Some(300_000)
    );
    assert_eq!(
        gemini_provider_core_retry_delay_ms(None, &retry_body, 0),
        Some(34_075)
    );
    assert_eq!(gemini_provider_core_retry_delay_ms(None, b"", 9), None);
    assert!(!gemini_provider_core_should_inline_rate_limit_retry(0));
    assert!(gemini_provider_core_should_inline_rate_limit_retry(30_000));
    assert!(!gemini_provider_core_should_inline_rate_limit_retry(30_001));
    assert_eq!(gemini_provider_core_invalid_stream_retry_delay_ms(0), 1_000);
    assert_eq!(
        gemini_provider_core_invalid_stream_retry_delay_ms(10),
        256_000
    );
    assert!(gemini_provider_core_should_rotate_after_quota_response(
        429, true, false, false, 0, 2
    ));
    assert!(!gemini_provider_core_should_rotate_after_quota_response(
        429, false, false, false, 0, 2
    ));
    assert!(!gemini_provider_core_should_rotate_after_quota_response(
        429, true, true, false, 0, 2
    ));
    assert!(gemini_provider_core_should_rotate_after_quota_response(
        429, true, true, true, 0, 2
    ));
    assert!(!gemini_provider_core_should_rotate_after_quota_response(
        429, true, false, false, 1, 2
    ));
}

#[test]
fn gemini_provider_core_normalizes_gemini_errors_to_openai_shape() {
    let body = serde_json::to_vec(&serde_json::json!({
        "error": {
            "code": 429,
            "message": "Quota exhausted.",
            "details": [{
                "@type": "type.googleapis.com/google.rpc.ErrorInfo",
                "reason": "QUOTA_EXHAUSTED"
            }]
        }
    }))
    .unwrap();

    let normalized = gemini_provider_core_normalized_error_body(429, &body).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&normalized).unwrap();

    assert!(gemini_provider_core_body_has_terminal_quota(&body));
    assert_eq!(value["error"]["type"], "insufficient_quota");
    assert_eq!(value["error"]["code"], "insufficient_quota");
    assert_eq!(value["error"]["message"], "Quota exhausted.");

    let normalized = gemini_provider_core_normalized_error_body(429, b"try later").unwrap();
    let value: serde_json::Value = serde_json::from_slice(&normalized).unwrap();
    assert_eq!(value["error"]["type"], "rate_limit_error");
    assert_eq!(value["error"]["code"], "rate_limit_exceeded");
    assert_eq!(value["error"]["message"], "try later");
}

#[test]
fn gemini_provider_core_extracts_binding_call_ids() {
    let request = serde_json::json!({
        "input": [
            {"type": "function_call_output", "call_id": "call_1"},
            {"type": "custom_tool_call_output", "tool_call_id": "call_2"},
            {"type": "mcp_tool_result", "id": "call_3"},
            {"type": "message", "id": "ignored"},
            {"type": "mcp_call_output", "call_id": " "}
        ]
    });
    assert_eq!(
        gemini_provider_core_tool_output_call_ids_from_request(&request),
        vec!["call_1", "call_2", "call_3"]
    );

    let response = serde_json::json!({
        "id": "resp_1",
        "output": [
            {"type": "function_call", "call_id": "call_4"},
            {"type": "custom_tool_call", "id": "call_5"},
            {"type": "message", "id": "ignored"}
        ]
    });
    assert_eq!(
        gemini_provider_core_response_id_from_responses_value(&response).as_deref(),
        Some("resp_1")
    );
    assert_eq!(
        gemini_provider_core_tool_call_ids_from_responses_value(&response),
        vec!["call_4", "call_5"]
    );
    assert_eq!(
        gemini_provider_core_response_bindings_from_body(&serde_json::to_vec(&response).unwrap()),
        Some((
            "resp_1".to_string(),
            vec!["call_4".to_string(), "call_5".to_string()]
        ))
    );
    assert_eq!(
        gemini_provider_core_response_bindings_from_body(b"{}"),
        None
    );
}

#[test]
fn gemini_provider_core_parses_live_audio_config_and_codecs() {
    assert_eq!(
        gemini_provider_core_live_audio_rate_from_mime("audio/pcm;rate=16000"),
        Some(16_000)
    );
    assert_eq!(
        gemini_provider_core_live_audio_config_from_value(&serde_json::json!("audio/pcmu"))
            .unwrap()
            .format,
        GeminiProviderCoreLiveAudioFormat::G711Ulaw
    );
    assert_eq!(
        gemini_provider_core_live_audio_config_from_value(&serde_json::json!({
            "format": "audio/pcma",
            "sampleRate": 12_000
        }))
        .unwrap()
        .rate,
        12_000
    );

    let session = serde_json::json!({
        "audio": {"input": {"format": "pcm16", "rate": 24_000}},
        "outputAudioFormat": "audio/pcm;rate=16000"
    });
    let session = session.as_object().unwrap();
    assert_eq!(
        gemini_provider_core_live_session_audio_config(session, "input")
            .unwrap()
            .rate,
        24_000
    );
    assert_eq!(
        gemini_provider_core_live_legacy_session_audio_config(
            session,
            "output_audio_format",
            "outputAudioFormat"
        )
        .unwrap()
        .rate,
        16_000
    );

    assert_eq!(gemini_provider_core_live_decode_ulaw(0xff), 0);
    assert_eq!(gemini_provider_core_live_decode_alaw(0xd5), 8);
    assert_eq!(
        gemini_provider_core_live_input_audio_payload(
            "abc",
            GeminiProviderCoreLiveAudioFormat::Pcm16,
            24_000
        )
        .unwrap()
        .mime_type,
        "audio/pcm;rate=24000"
    );
    assert_eq!(
        gemini_provider_core_live_input_audio_payload(
            "/w==",
            GeminiProviderCoreLiveAudioFormat::G711Ulaw,
            8_000
        )
        .unwrap()
        .data,
        "AAA="
    );
}

#[test]
fn gemini_provider_core_maps_live_messages() {
    let tool = serde_json::json!({
        "type": "function",
        "name": "lookup",
        "description": "Lookup data",
        "parameters": {"type": "object", "properties": {"id": {"type": "string"}}}
    });
    let declaration = gemini_provider_core_live_function_declaration(&tool).unwrap();
    assert_eq!(declaration["name"], "lookup");
    assert_eq!(declaration["description"], "Lookup data");

    let session = serde_json::json!({
        "model": "gemini-live",
        "output_modalities": ["audio", "text"],
        "instructions": "Be brief.",
        "tools": [tool]
    });
    let setup = gemini_provider_core_live_setup_message(
        session.as_object().unwrap(),
        "gemini-default",
        Some("models/gemini-configured"),
    );
    assert_eq!(setup["setup"]["model"], "models/gemini-configured");
    assert_eq!(
        setup["setup"]["generation_config"]["response_modalities"][0],
        "AUDIO"
    );
    assert_eq!(
        setup["setup"]["system_instruction"]["parts"][0]["text"],
        "Be brief."
    );
    assert_eq!(
        setup["setup"]["tools"][0]["function_declarations"][0]["name"],
        "lookup"
    );
    assert_eq!(
        gemini_provider_core_live_realtime_audio_message(
            "abc".to_string(),
            "audio/pcm".to_string()
        )["realtime_input"]["audio"]["data"],
        "abc"
    );
    assert_eq!(
        gemini_provider_core_live_audio_stream_end_message()["realtime_input"]["audio_stream_end"],
        true
    );
    assert_eq!(
        gemini_provider_core_live_input_audio_cleared_event()["type"],
        "input_audio_buffer.cleared"
    );
    assert_eq!(
        gemini_provider_core_live_response_created_event("resp")["response"]["id"],
        "resp"
    );
    assert_eq!(
        gemini_provider_core_live_response_cancelled_event("resp")["type"],
        "response.cancelled"
    );
    assert_eq!(
        gemini_provider_core_live_conversation_item_truncated_event(&serde_json::json!({
            "item_id": "item",
            "content_index": 1,
            "audio_end_ms": 2
        }))["audio_end_ms"],
        2
    );
    assert_eq!(
        gemini_provider_core_live_conversation_item_deleted_event(&serde_json::json!({
            "item_id": "item"
        }))["item_id"],
        "item"
    );
    assert_eq!(
        gemini_provider_core_live_unsupported_event_error("bad")["error"]["message"],
        "Unsupported Codex realtime event type: bad"
    );
    assert_eq!(
        gemini_provider_core_live_session_updated_event()["session"]["id"],
        "sess_gemini_live"
    );
    assert_eq!(
        gemini_provider_core_live_error_event(&serde_json::json!({"message": "boom"}))["error"]["message"],
        "boom"
    );
    assert_eq!(
        gemini_provider_core_live_provider_stream_error("stream broke")["error"]["type"],
        "provider_stream_error"
    );
    assert_eq!(
        gemini_provider_core_live_binary_frame_error()["error"]["message"],
        "Gemini Live bridge expects JSON text websocket frames."
    );
    assert_eq!(
        gemini_provider_core_live_function_call_done_event(
            "call_1",
            "lookup",
            &serde_json::json!({"id": "42"})
        )["item"]["arguments"],
        "{\"id\":\"42\"}"
    );
    assert_eq!(
        gemini_provider_core_live_input_audio_transcription_delta_event("item", "hel")["delta"],
        "hel"
    );
    assert_eq!(
        gemini_provider_core_live_input_audio_transcription_completed_event("item", "hello")["transcript"],
        "hello"
    );
    assert_eq!(
        gemini_provider_core_live_output_audio_transcript_delta_event("item", "resp", "hi")["response_id"],
        "resp"
    );
    assert_eq!(
        gemini_provider_core_live_output_audio_transcript_done_event("item", "resp", "hello")["transcript"],
        "hello"
    );
    assert_eq!(
        gemini_provider_core_live_output_text_delta_event("item", "resp", "hi")["delta"],
        "hi"
    );
    assert_eq!(
        gemini_provider_core_live_output_text_done_event("item", "resp", "hello")["text"],
        "hello"
    );
    assert_eq!(
        gemini_provider_core_live_output_audio_delta_event("item", "resp", "abc", 24_000)["sample_rate"],
        24_000
    );
    assert_eq!(
        gemini_provider_core_live_response_done_event("resp")["response"]["id"],
        "resp"
    );

    let content = serde_json::json!({
        "item": {
            "type": "message",
            "content": [{"text": "hello"}, {"text": "world"}]
        }
    });
    let message = gemini_provider_core_live_client_content_message(&content).unwrap();
    assert_eq!(
        message["client_content"]["turns"][0]["parts"][0]["text"],
        "hello\nworld"
    );

    let mut tool_names = HashMap::new();
    tool_names.insert("call_1".to_string(), "lookup".to_string());
    let output = serde_json::json!({
        "item": {
            "type": "function_call_output",
            "call_id": "call_1",
            "output": {"ok": true}
        }
    });
    let (call_id, response) =
        gemini_provider_core_live_tool_response_message(&output, &tool_names).unwrap();
    assert_eq!(call_id, "call_1");
    assert_eq!(
        response["tool_response"]["function_responses"][0]["name"],
        "lookup"
    );

    let server = serde_json::json!({
        "server_content": {
            "turn_complete": true,
            "input_transcription": {"text": "hello"}
        }
    });
    assert!(gemini_provider_core_live_server_turn_complete(&server));
    let content = server.get("server_content").unwrap();
    assert_eq!(
        gemini_provider_core_live_transcription_text(
            content,
            "inputTranscription",
            "input_transcription"
        ),
        Some("hello")
    );
    assert_eq!(
        gemini_provider_core_live_transcript_delta("hello", "hello world"),
        " world"
    );
}

#[test]
fn gemini_provider_core_decides_precommit_stream_visibility() {
    let data = vec![
        serde_json::json!({
            "responseId": "resp_text",
            "candidates": [{
                "content": {"parts": [{"text": "hi"}]}
            }]
        })
        .to_string(),
    ];
    let mut probe = GeminiProviderCorePrecommitProbe::default();
    assert_eq!(
        gemini_provider_core_precommit_decision_for_data_lines(&data, &mut probe),
        GeminiProviderCorePrecommitDecision::Commit
    );

    let data = vec![
        serde_json::json!({
            "responseId": "resp_empty",
            "candidates": [{
                "content": {"parts": []},
                "finishReason": "STOP"
            }]
        })
        .to_string(),
    ];
    let mut probe = GeminiProviderCorePrecommitProbe::default();
    assert_eq!(
        gemini_provider_core_precommit_decision_for_data_lines(&data, &mut probe),
        GeminiProviderCorePrecommitDecision::RetryableInvalid("gemini_empty_response".to_string())
    );
}

#[test]
fn gemini_provider_core_rewrites_semantic_compact_request() {
    let body = serde_json::to_vec(&serde_json::json!({
        "model": "auto",
        "stream": true,
        "store": true,
        "previous_response_id": "resp_old",
        "tools": [{"type": "function", "name": "shell"}],
        "input": [{"type": "message", "role": "user", "content": "summarize"}]
    }))
    .unwrap();

    let rewritten = gemini_provider_core_semantic_compact_request_body(&body).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&rewritten).unwrap();

    assert_eq!(value["model"], crate::PRODEX_GEMINI_CHAT_COMPRESSION_MODEL);
    assert_eq!(value["stream"], false);
    assert_eq!(value["store"], false);
    assert_eq!(value["parallel_tool_calls"], false);
    assert_eq!(value["prodex_gemini_compaction"], true);
    assert!(value.get("previous_response_id").is_none());
    assert!(value.get("tools").is_none());
    assert!(
        value["instructions"]
            .as_str()
            .unwrap()
            .contains("durable continuation summary")
    );
    assert_eq!(value["input"].as_array().unwrap().len(), 2);
}

#[test]
fn gemini_provider_core_policy_helpers_match_runtime_behavior() {
    assert_eq!(
        gemini_provider_core_normalize_tool_name("mcp.shell-command"),
        "shell_command"
    );
    assert!(gemini_provider_core_tool_is_mutating("run_shell_command"));
    assert!(gemini_provider_core_tool_is_mutating("mcp.apply-patch"));
    assert!(!gemini_provider_core_tool_is_mutating("read_file"));
    assert_eq!(
        gemini_provider_core_tool_call_command_text(&serde_json::json!({
            "shellCommand": "cargo test"
        })),
        "cargo test"
    );
    assert_eq!(
        gemini_provider_core_tool_call_command_text(&serde_json::json!("echo hi")),
        "echo hi"
    );
}

#[test]
fn gemini_provider_core_builds_compact_summaries() {
    let body = serde_json::to_vec(&serde_json::json!({
        "model": "auto",
        "input": [
            {
                "type": "message",
                "role": "user",
                "content": [{"type": "input_text", "text": "Fix compact fallback."}]
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

    let summary = gemini_provider_core_local_compact_summary(&body);
    assert!(summary.contains("Local Gemini compact fallback summary."));
    assert!(summary.contains("Fix compact fallback."));
    assert!(summary.contains("tool call shell (call_1)"));
    assert!(summary.contains("tool output call_1: tests passed"));

    let continuation =
        gemini_provider_core_semantic_compact_continuation_summary("Keep going.", &body);
    assert!(continuation.contains("Active user request that must still be completed"));
    assert!(continuation.contains("Latest tool result after the active request"));
    assert!(continuation.contains("Semantic continuation summary:\nKeep going."));
}

#[test]
fn gemini_provider_core_extracts_semantic_compact_summary() {
    let chat = serde_json::json!({
        "choices": [{
            "message": {"content": " Chat summary. "}
        }]
    });
    assert_eq!(
        gemini_provider_core_semantic_compact_summary(&chat, 7),
        "Chat summary."
    );

    let generate = serde_json::json!({
        "candidates": [{
            "content": {
                "parts": [{"text": "Gemini summary."}]
            }
        }]
    });
    assert_eq!(
        gemini_provider_core_semantic_compact_summary(&generate, 7),
        "Gemini summary."
    );
}
