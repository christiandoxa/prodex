use super::{
    deepseek_provider_core_apply_reasoning_from_responses_request,
    deepseek_provider_core_apply_strict_function_schema,
    deepseek_provider_core_chat_assistant_messages_from_response_value,
    deepseek_provider_core_chat_role, deepseek_provider_core_dedup_and_validate_function_tools,
    deepseek_provider_core_ensure_json_prompt_instruction,
    deepseek_provider_core_first_function_call_output_call_id,
    deepseek_provider_core_function_tool_name, deepseek_provider_core_history_has_system_message,
    deepseek_provider_core_history_has_tool_call,
    deepseek_provider_core_insert_primitive_request_fields,
    deepseek_provider_core_is_generic_function_tool, deepseek_provider_core_json_string,
    deepseek_provider_core_json_string_at_path, deepseek_provider_core_merge_response_metadata,
    deepseek_provider_core_message_signatures,
    deepseek_provider_core_messages_from_responses_request,
    deepseek_provider_core_normalize_thinking_tool_call_messages,
    deepseek_provider_core_note_thinking_tool_choice_omission,
    deepseek_provider_core_push_message_from_responses_item,
    deepseek_provider_core_reject_beta_completion_fields,
    deepseek_provider_core_reject_unsupported_request_fields,
    deepseek_provider_core_repair_tool_call_adjacency, deepseek_provider_core_request_body,
    deepseek_provider_core_response_format_from_responses_request,
    deepseek_provider_core_response_metadata_from_responses_request,
    deepseek_provider_core_responses_content_text,
    deepseek_provider_core_responses_content_text_value,
    deepseek_provider_core_rtk_wrapped_tool_arguments, deepseek_provider_core_simple_request,
    deepseek_provider_core_stop_from_responses_request, deepseek_provider_core_system_message,
    deepseek_provider_core_thinking_enabled, deepseek_provider_core_tool_call_ids,
    deepseek_provider_core_tool_name_from_tool_object, deepseek_provider_core_tool_output_call_ids,
    deepseek_provider_core_top_logprobs_from_responses_request,
    deepseek_provider_core_user_id_from_responses_request, deepseek_provider_core_user_message,
    deepseek_provider_core_validate_function_name,
    deepseek_provider_core_validate_function_parameters,
    deepseek_provider_core_validate_reasoning_shape,
    deepseek_provider_core_validate_supported_input_item,
    deepseek_provider_core_validate_tool_choice_name,
    deepseek_provider_core_validate_tool_choice_shape,
    deepseek_provider_core_validate_tool_choice_target,
    deepseek_provider_core_validate_tools_shape,
    deepseek_provider_core_validate_web_search_options,
    deepseek_provider_core_validate_web_search_tool_context_size,
};
use crate::{
    ProviderEndpoint, ProviderId, ProviderTransformLoss, ProviderTransformResult,
    ProviderWireFormat,
};
use std::collections::BTreeSet;

#[test]
fn deepseek_provider_core_simple_request_accepts_plain_text_first_turn() {
    let body = serde_json::to_vec(&serde_json::json!({
        "model": "deepseek-chat",
        "input": "hello",
        "stream": true,
        "temperature": 0.2
    }))
    .unwrap();
    assert!(deepseek_provider_core_simple_request(&body, |_| false));
}

#[test]
fn deepseek_provider_core_simple_request_rejects_bound_continuation_and_nonsimple_tools() {
    let continuation = serde_json::to_vec(&serde_json::json!({
        "model": "deepseek-chat",
        "input": "hello",
        "previous_response_id": "resp_1"
    }))
    .unwrap();
    assert!(deepseek_provider_core_simple_request(&continuation, |_| {
        false
    }));
    assert!(!deepseek_provider_core_simple_request(
        &continuation,
        |id| id == "resp_1"
    ));

    let tools = serde_json::to_vec(&serde_json::json!({
        "model": "deepseek-chat",
        "input": "hello",
        "tools": [{"type":"web_search_preview"}]
    }))
    .unwrap();
    assert!(!deepseek_provider_core_simple_request(&tools, |_| false));
}

#[test]
fn deepseek_provider_core_request_body_allows_only_retryable_losses() {
    let result = ProviderTransformResult {
        provider: ProviderId::DeepSeek,
        endpoint: ProviderEndpoint::Responses,
        from_format: ProviderWireFormat::OpenAiResponses,
        to_format: ProviderWireFormat::OpenAiChatCompletions,
        body: Some(br#"{"model":"deepseek-chat"}"#.to_vec()),
        headers: Default::default(),
        metadata: Default::default(),
        loss: ProviderTransformLoss::Lossless,
    };
    assert_eq!(
        deepseek_provider_core_request_body(&result),
        Some(br#"{"model":"deepseek-chat"}"#.to_vec())
    );
}

#[test]
fn deepseek_provider_core_rtk_wrapped_tool_arguments_wraps_exec_commands() {
    let arguments = deepseek_provider_core_rtk_wrapped_tool_arguments(
        "exec",
        r#"{"cmd":"claw-compactor benchmark /workspace --json"}"#,
    );
    let arguments: serde_json::Value = serde_json::from_str(&arguments).unwrap();
    assert_eq!(
        arguments["cmd"],
        "rtk claw-compactor benchmark /workspace --json"
    );

    let arguments = deepseek_provider_core_rtk_wrapped_tool_arguments("shell", r#"{"cmd":"ls"}"#);
    let arguments: serde_json::Value = serde_json::from_str(&arguments).unwrap();
    assert_eq!(arguments["cmd"], "rtk ls");

    let arguments = deepseek_provider_core_rtk_wrapped_tool_arguments("exec", r#"{"cmd":"pwd"}"#);
    let arguments: serde_json::Value = serde_json::from_str(&arguments).unwrap();
    assert_eq!(arguments["cmd"], "pwd");
}

#[test]
fn deepseek_provider_core_responses_content_text_preserves_runtime_shape() {
    assert_eq!(
        deepseek_provider_core_responses_content_text(Some(&serde_json::json!("plain"))),
        "plain"
    );
    assert_eq!(
        deepseek_provider_core_responses_content_text_value(&serde_json::json!([
            "a",
            {"input_text": "b"},
            {"output_text": "c"},
            {"ignored": true}
        ])),
        "a\nb\nc"
    );
    assert_eq!(
        deepseek_provider_core_responses_content_text(Some(&serde_json::json!({"x": 1}))),
        "{\"x\":1}"
    );
    assert_eq!(deepseek_provider_core_responses_content_text(None), "");
}

#[test]
fn deepseek_provider_core_extracts_chat_assistant_messages_from_response() {
    let messages =
        deepseek_provider_core_chat_assistant_messages_from_response_value(&serde_json::json!({
            "choices": [{
                "message": {
                    "content": "",
                    "reasoning_content": "thinking",
                    "tool_calls": [{
                        "id": "call_1",
                        "type": "function",
                        "function": {
                            "name": "shell",
                            "arguments": "{\"cmd\":\"ls\"}"
                        }
                    }]
                }
            }]
        }));

    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0]["role"], "assistant");
    assert_eq!(messages[0]["content"], "");
    assert_eq!(messages[0]["reasoning_content"], "thinking");
    let arguments = messages[0]["tool_calls"][0]["function"]["arguments"]
        .as_str()
        .unwrap();
    let arguments: serde_json::Value = serde_json::from_str(arguments).unwrap();
    assert_eq!(arguments["cmd"], "rtk ls");
}

#[test]
fn deepseek_provider_core_omits_empty_chat_assistant_messages() {
    assert!(
        deepseek_provider_core_chat_assistant_messages_from_response_value(
            &serde_json::json!({"choices": [{"message": {"content": ""}}]})
        )
        .is_empty()
    );
    assert!(
        deepseek_provider_core_chat_assistant_messages_from_response_value(
            &serde_json::json!({"choices": []})
        )
        .is_empty()
    );
}

#[test]
fn deepseek_provider_core_merges_response_metadata() {
    let mut response = serde_json::json!({
        "id": "resp_1",
        "metadata": {
            "deepseek": {"existing": true},
            "keep": "yes"
        }
    });

    deepseek_provider_core_merge_response_metadata(
        &mut response,
        Some(serde_json::json!({
            "deepseek": {"added": true},
            "new": {"nested": 1}
        })),
    );

    assert_eq!(response["metadata"]["deepseek"]["existing"], true);
    assert_eq!(response["metadata"]["deepseek"]["added"], true);
    assert_eq!(response["metadata"]["keep"], "yes");
    assert_eq!(response["metadata"]["new"]["nested"], 1);

    deepseek_provider_core_merge_response_metadata(&mut response, Some(serde_json::json!(true)));
    assert_eq!(response["metadata"]["keep"], "yes");
}

#[test]
fn deepseek_provider_core_shapes_basic_chat_messages() {
    assert_eq!(
        deepseek_provider_core_system_message("rules"),
        serde_json::json!({"role": "system", "content": "rules"})
    );
    assert_eq!(
        deepseek_provider_core_user_message("hello"),
        serde_json::json!({"role": "user", "content": "hello"})
    );
}

#[test]
fn deepseek_provider_core_messages_from_responses_request_replays_history_and_input() {
    let value = serde_json::json!({
        "input": "hello",
        "instructions": "Please be concise."
    });
    let history = vec![serde_json::json!({"role":"user","content":"read docs"})];
    let messages =
        deepseek_provider_core_messages_from_responses_request(&value, &history, false, "deepseek")
            .unwrap()
            .unwrap();

    assert_eq!(messages[0]["role"], "system");
    assert_eq!(messages[0]["content"], "Please be concise.");
    assert_eq!(messages[1]["role"], "user");
    assert_eq!(messages[1]["content"], "read docs");
    assert_eq!(messages[2]["role"], "user");
    assert_eq!(messages[2]["content"], "hello");
}

#[test]
fn deepseek_provider_core_messages_from_responses_request_rejects_bad_input_items() {
    let value = serde_json::json!({
        "input": [{"role": "user", "content": "hi"}, 42]
    });
    assert!(
        deepseek_provider_core_messages_from_responses_request(&value, &[], false, "deepseek")
            .is_err()
    );
}

#[test]
fn deepseek_provider_core_json_string_helpers_preserve_runtime_shape() {
    let value = serde_json::json!({
        "name": "top",
        "function": {"name": "nested"},
        "other": 1
    });
    let object = value.as_object().unwrap();

    assert_eq!(
        deepseek_provider_core_json_string(object, &["missing", "name"]),
        Some("top".to_string())
    );
    assert_eq!(
        deepseek_provider_core_json_string_at_path(object, &["function", "name"]),
        Some("nested".to_string())
    );
    assert_eq!(
        deepseek_provider_core_json_string_at_path(object, &[]),
        None
    );
}

#[test]
fn deepseek_provider_core_normalizes_thinking_tool_call_messages() {
    let mut messages = vec![
        serde_json::json!({
            "role": "assistant",
            "tool_calls": [{"id": "call_1"}]
        }),
        serde_json::json!({
            "role": "assistant",
            "content": null,
            "reasoning_content": "think",
            "tool_calls": [{"id": "call_2"}]
        }),
        serde_json::json!({"role": "user", "content": null}),
    ];

    deepseek_provider_core_normalize_thinking_tool_call_messages(&mut messages);

    assert_eq!(messages[0]["content"], "");
    assert_eq!(messages[0]["reasoning_content"], "");
    assert_eq!(messages[1]["content"], "");
    assert_eq!(messages[1]["reasoning_content"], "think");
    assert!(messages[2]["content"].is_null());
}

#[test]
fn deepseek_provider_core_repairs_tool_call_adjacency() {
    let mut messages = vec![
        serde_json::json!({
            "role": "assistant",
            "tool_calls": [
                {"id": "call_1", "type": "function"},
                {"id": "call_missing", "type": "function"}
            ]
        }),
        serde_json::json!({"role": "user", "content": "next"}),
        serde_json::json!({
            "role": "tool",
            "tool_call_id": "call_1",
            "content": "ok"
        }),
    ];

    deepseek_provider_core_repair_tool_call_adjacency(&mut messages);

    assert_eq!(messages.len(), 3);
    assert_eq!(messages[0]["content"], "");
    assert_eq!(messages[0]["tool_calls"].as_array().unwrap().len(), 1);
    assert_eq!(messages[1]["role"], "tool");
    assert_eq!(messages[2]["role"], "user");
}

#[test]
fn deepseek_provider_core_maps_reasoning_effort() {
    let value = serde_json::json!({"reasoning": {"effort": "xhigh"}});
    let mut request = serde_json::Map::new();

    deepseek_provider_core_apply_reasoning_from_responses_request(
        &value,
        &mut request,
        "DeepSeek",
        false,
    )
    .unwrap();

    assert_eq!(request["thinking"], serde_json::json!({"type": "enabled"}));
    assert_eq!(request["reasoning_effort"], "max");
    assert!(deepseek_provider_core_thinking_enabled(&value));

    request.clear();
    deepseek_provider_core_apply_reasoning_from_responses_request(
        &serde_json::json!({"reasoning_effort": "minimal"}),
        &mut request,
        "DeepSeek",
        false,
    )
    .unwrap();
    assert_eq!(request["thinking"], serde_json::json!({"type": "disabled"}));
    assert!(request.get("reasoning_effort").is_none());
}

#[test]
fn deepseek_provider_core_maps_gemini_reasoning_effort() {
    let mut request = serde_json::Map::new();
    deepseek_provider_core_apply_reasoning_from_responses_request(
        &serde_json::json!({"reasoning": {"effort": "xhigh"}}),
        &mut request,
        "Gemini OpenAI-compatible",
        true,
    )
    .unwrap();

    assert_eq!(request["reasoning_effort"], "high");
    assert!(
        deepseek_provider_core_apply_reasoning_from_responses_request(
            &serde_json::json!({"reasoning": {"effort": "weird"}}),
            &mut request,
            "Gemini OpenAI-compatible",
            true,
        )
        .unwrap_err()
        .contains("Gemini OpenAI-compatible reasoning effort is not supported")
    );
}

#[test]
fn deepseek_provider_core_validates_reasoning_shape() {
    assert!(
        deepseek_provider_core_validate_reasoning_shape(
            &serde_json::json!({"reasoning": true}),
            "DeepSeek",
        )
        .unwrap_err()
        .contains("DeepSeek reasoning must be an object")
    );
    assert!(
        deepseek_provider_core_validate_reasoning_shape(
            &serde_json::json!({"reasoning": {"summary": "auto"}}),
            "DeepSeek",
        )
        .unwrap_err()
        .contains("DeepSeek reasoning.summary is not supported")
    );
    assert!(
        deepseek_provider_core_validate_reasoning_shape(
            &serde_json::json!({"reasoning_effort": 1}),
            "DeepSeek",
        )
        .unwrap_err()
        .contains("DeepSeek reasoning_effort must be a string")
    );
}

#[test]
fn deepseek_provider_core_maps_primitive_request_fields() {
    let value = serde_json::json!({
        "temperature": 0.2,
        "top_p": 0.9,
        "max_output_tokens": 123,
        "logprobs": true,
        "top_logprobs": 3,
        "stop": ["a", "b"],
        "user_id": " user_1 "
    });
    let mut request = serde_json::Map::new();

    deepseek_provider_core_insert_primitive_request_fields(&value, &mut request, "DeepSeek")
        .unwrap();

    assert_eq!(request["temperature"], 0.2);
    assert_eq!(request["top_p"], 0.9);
    assert_eq!(request["max_tokens"], 123);
    assert_eq!(request["logprobs"], true);
    assert_eq!(
        deepseek_provider_core_top_logprobs_from_responses_request(&value, "DeepSeek").unwrap(),
        Some(serde_json::json!(3))
    );
    assert_eq!(
        deepseek_provider_core_stop_from_responses_request(&value, "DeepSeek").unwrap(),
        Some(serde_json::json!(["a", "b"]))
    );
    assert_eq!(
        deepseek_provider_core_user_id_from_responses_request(&value, "DeepSeek").unwrap(),
        Some("user_1".to_string())
    );
}

#[test]
fn deepseek_provider_core_rejects_invalid_primitive_request_fields() {
    assert!(
        deepseek_provider_core_stop_from_responses_request(
            &serde_json::json!({"stop": [1]}),
            "DeepSeek",
        )
        .unwrap_err()
        .contains("DeepSeek stop sequences must be strings")
    );
    assert!(
        deepseek_provider_core_insert_primitive_request_fields(
            &serde_json::json!({"max_tokens": 0}),
            &mut serde_json::Map::new(),
            "DeepSeek",
        )
        .unwrap_err()
        .contains("DeepSeek max_tokens must be a positive integer")
    );
    assert!(
        deepseek_provider_core_top_logprobs_from_responses_request(
            &serde_json::json!({"top_logprobs": 1}),
            "DeepSeek",
        )
        .unwrap_err()
        .contains("DeepSeek top_logprobs requires logprobs=true")
    );
    assert!(
        deepseek_provider_core_user_id_from_responses_request(
            &serde_json::json!({"user": "bad!"}),
            "DeepSeek",
        )
        .unwrap_err()
        .contains("DeepSeek user_id must use only letters")
    );
}

#[test]
fn deepseek_provider_core_rejects_unsupported_request_fields() {
    assert!(
        deepseek_provider_core_reject_unsupported_request_fields(
            &serde_json::json!({"frequency_penalty": 1}),
            "DeepSeek",
        )
        .unwrap_err()
        .contains("DeepSeek frequency_penalty is deprecated")
    );
    assert!(
        deepseek_provider_core_reject_unsupported_request_fields(
            &serde_json::json!({"stream_options": {"include_usage": true}}),
            "DeepSeek",
        )
        .unwrap_err()
        .contains("DeepSeek stream_options requires stream=true")
    );
    assert!(
        deepseek_provider_core_reject_unsupported_request_fields(
            &serde_json::json!({"modalities": ["text", "audio"]}),
            "DeepSeek",
        )
        .unwrap_err()
        .contains("DeepSeek Responses adapter only supports text modality")
    );
    deepseek_provider_core_reject_unsupported_request_fields(
        &serde_json::json!({
            "stream": true,
            "stream_options": {"include_usage": true},
            "parallel_tool_calls": true,
            "modalities": ["text"],
            "truncation": "disabled",
            "store": false,
            "background": false,
            "include": []
        }),
        "DeepSeek",
    )
    .unwrap();
}

#[test]
fn deepseek_provider_core_rejects_beta_completion_fields() {
    assert!(
        deepseek_provider_core_reject_beta_completion_fields(
            &serde_json::json!({"prefix": "a"}),
            "DeepSeek",
        )
        .unwrap_err()
        .contains("DeepSeek chat prefix completion requires the beta chat endpoint")
    );
    assert!(
        deepseek_provider_core_reject_beta_completion_fields(
            &serde_json::json!({"suffix": "b"}),
            "DeepSeek",
        )
        .unwrap_err()
        .contains("DeepSeek FIM suffix completion requires the beta /completions endpoint")
    );
    assert!(
        deepseek_provider_core_reject_beta_completion_fields(
            &serde_json::json!({"prompt": "c"}),
            "DeepSeek",
        )
        .unwrap_err()
        .contains("DeepSeek prompt completions require the beta /completions endpoint")
    );
}

#[test]
fn deepseek_provider_core_maps_response_format_and_metadata() {
    let value = serde_json::json!({
        "response_format": {"type": "json_schema"},
        "metadata": {"tag": "one"},
        "client_metadata": {"client": true},
        "prompt_cache_key": " cache-key ",
        "prompt_cache_retention": "24h"
    });

    assert_eq!(
        deepseek_provider_core_response_format_from_responses_request(&value, "DeepSeek").unwrap(),
        Some(serde_json::json!({"type": "json_object"}))
    );

    let metadata = deepseek_provider_core_response_metadata_from_responses_request(
        &value, "DeepSeek", "deepseek",
    )
    .unwrap()
    .unwrap();

    assert_eq!(metadata["tag"], "one");
    assert_eq!(metadata["client_metadata"]["client"], true);
    assert_eq!(metadata["prompt_cache_key"], " cache-key ");
    assert_eq!(metadata["prompt_cache_retention"], "24h");
    assert_eq!(
        metadata["deepseek"]["degraded_response_format"]["from"],
        "json_schema"
    );
}

#[test]
fn deepseek_provider_core_rejects_invalid_response_format_and_metadata() {
    assert!(
        deepseek_provider_core_response_format_from_responses_request(
            &serde_json::json!({"response_format": {"type": "xml"}}),
            "DeepSeek",
        )
        .unwrap_err()
        .contains("DeepSeek response_format type `xml` is not supported")
    );
    assert!(
        deepseek_provider_core_response_metadata_from_responses_request(
            &serde_json::json!({"metadata": []}),
            "DeepSeek",
            "deepseek",
        )
        .unwrap_err()
        .contains("DeepSeek request metadata must be an object")
    );
    assert!(
        deepseek_provider_core_response_metadata_from_responses_request(
            &serde_json::json!({"metadata": {"deepseek": true}}),
            "DeepSeek",
            "deepseek",
        )
        .unwrap_err()
        .contains("DeepSeek request metadata.deepseek must be an object")
    );
}

#[test]
fn deepseek_provider_core_notes_thinking_tool_choice_omission() {
    let mut metadata = None;

    deepseek_provider_core_note_thinking_tool_choice_omission(
        &serde_json::json!({"tool_choice": "required"}),
        true,
        "DeepSeek",
        "deepseek",
        &mut metadata,
    );

    let metadata = metadata.unwrap();
    assert_eq!(
        metadata["deepseek"]["omitted_tool_choice"]["from"],
        "required"
    );
    assert!(
        metadata["deepseek"]["omitted_tool_choice"]["reason"]
            .as_str()
            .unwrap()
            .contains("DeepSeek thinking mode")
    );
}

#[test]
fn deepseek_provider_core_ensures_json_prompt_instruction() {
    let mut messages = vec![serde_json::json!({
        "role": "user",
        "content": "hello"
    })];

    deepseek_provider_core_ensure_json_prompt_instruction(&mut messages);

    assert_eq!(messages[0]["role"], "system");
    assert_eq!(messages[0]["content"], "Respond with valid JSON only.");
    assert_eq!(messages.len(), 2);

    deepseek_provider_core_ensure_json_prompt_instruction(&mut messages);
    assert_eq!(messages.len(), 2);
}

#[test]
fn deepseek_provider_core_validates_tool_choice_and_names() {
    let choice = serde_json::json!({"type": "function", "function": {"name": "demo_tool"}});
    let mut tool_names = BTreeSet::new();
    tool_names.insert("demo_tool".to_string());

    assert_eq!(
        deepseek_provider_core_function_tool_name(&choice),
        Some("demo_tool".to_string())
    );
    deepseek_provider_core_validate_tool_choice_name(&choice, "DeepSeek").unwrap();
    deepseek_provider_core_validate_tool_choice_shape(
        &serde_json::json!({"tool_choice": choice}),
        false,
        "DeepSeek",
    )
    .unwrap();
    deepseek_provider_core_validate_tool_choice_target(
        &serde_json::json!({"function": {"name": "demo_tool"}}),
        &tool_names,
        "DeepSeek",
    )
    .unwrap();

    assert!(
        deepseek_provider_core_validate_function_name("bad.name", "DeepSeek")
            .unwrap_err()
            .contains("DeepSeek function tool names must use only letters")
    );
}

#[test]
fn deepseek_provider_core_validates_tool_shapes() {
    deepseek_provider_core_validate_tools_shape(
            &serde_json::json!({
                "tools": [
                    {"type": "function", "function": {"name": "demo_tool", "strict": false}},
                    {"type": "custom", "name": "custom_tool", "format": {"type": "text"}},
                    {"type": "namespace", "name": "ns", "tools": [{"type": "function", "name": "call"}]},
                    {"type": "mcp_toolset", "server_name": "server", "allowed_tools": ["search"]},
                    {"type": "mcp_function", "name": "search", "input_schema": {}},
                    {"type": "web_fetch"}
                ]
            }),
            true,
            "DeepSeek",
        )
        .unwrap();
}

#[test]
fn deepseek_provider_core_rejects_invalid_tool_shapes() {
    assert!(
        deepseek_provider_core_validate_tools_shape(
            &serde_json::json!({"tools": {"type": "function"}}),
            false,
            "DeepSeek",
        )
        .unwrap_err()
        .contains("DeepSeek tools must be an array")
    );
    assert!(
        deepseek_provider_core_validate_tools_shape(
            &serde_json::json!({"tools": [{"type": "custom", "name": "x", "strict": true}]}),
            false,
            "DeepSeek",
        )
        .unwrap_err()
        .contains("DeepSeek custom tools cannot preserve strict=true")
    );
    assert!(
        deepseek_provider_core_validate_tools_shape(
            &serde_json::json!({"tools": [{"type": "namespace", "name": "ns", "tools": []}]}),
            false,
            "DeepSeek",
        )
        .unwrap_err()
        .contains("DeepSeek namespace tool `ns` requires at least one tool")
    );
    assert!(
        deepseek_provider_core_validate_tools_shape(
            &serde_json::json!({"tools": [{"type": "file_search"}]}),
            false,
            "DeepSeek",
        )
        .unwrap_err()
        .contains("DeepSeek tool type `file_search` is not supported")
    );
}

#[test]
fn deepseek_provider_core_identifies_tool_object_names_and_generic_tools() {
    let object_tool = serde_json::json!({"function": {"name": "nested"}});
    assert_eq!(
        deepseek_provider_core_tool_name_from_tool_object(object_tool.as_object().unwrap()),
        Some("nested".to_string())
    );

    assert!(deepseek_provider_core_is_generic_function_tool(
        &serde_json::json!({
            "type": "function",
            "function": {
                "name": "generic",
                "parameters": {"type": "object", "additionalProperties": true}
            }
        })
    ));
}

#[test]
fn deepseek_provider_core_applies_strict_function_schema() {
    let mut tool = serde_json::json!({
        "type": "function",
        "function": {
            "name": "lookup",
            "parameters": {
                "type": "object",
                "properties": {
                    "query": {"type": "string"},
                    "limit": {"type": "integer"}
                }
            }
        }
    });

    deepseek_provider_core_validate_function_parameters(&tool, "lookup", "DeepSeek").unwrap();
    deepseek_provider_core_apply_strict_function_schema(&mut tool, "DeepSeek").unwrap();

    let function = &tool["function"];
    assert_eq!(function["strict"], true);
    assert_eq!(function["parameters"]["required"][0], "limit");
    assert_eq!(function["parameters"]["required"][1], "query");
    assert_eq!(function["parameters"]["additionalProperties"], false);
}

#[test]
fn deepseek_provider_core_rejects_invalid_strict_function_schema() {
    assert!(
        deepseek_provider_core_validate_function_parameters(
            &serde_json::json!({
                "function": {"name": "lookup", "parameters": true}
            }),
            "lookup",
            "DeepSeek",
        )
        .unwrap_err()
        .contains("DeepSeek function tool `lookup` parameters must be an object")
    );

    let mut tool = serde_json::json!({
        "function": {
            "name": "lookup",
            "parameters": {
                "type": "object",
                "properties": {
                    "query": {"type": "string", "pattern": "^[a-z]+$"}
                }
            }
        }
    });

    assert!(
        deepseek_provider_core_apply_strict_function_schema(&mut tool, "DeepSeek")
            .unwrap_err()
            .contains(
                "DeepSeek strict tool schema `lookup.query` uses unsupported keyword `pattern`"
            )
    );
}

#[test]
fn deepseek_provider_core_dedups_function_tools() {
    let generic = serde_json::json!({
        "type": "function",
        "function": {
            "name": "lookup",
            "parameters": {"type": "object", "additionalProperties": true}
        }
    });
    let specific = serde_json::json!({
        "type": "function",
        "function": {
            "name": "lookup",
            "parameters": {"type": "object", "properties": {"query": {"type": "string"}}}
        }
    });

    let tools = deepseek_provider_core_dedup_and_validate_function_tools(
        vec![generic.clone(), generic.clone(), specific.clone()],
        false,
        "DeepSeek",
    )
    .unwrap();

    assert_eq!(tools, vec![specific]);
}

#[test]
fn deepseek_provider_core_validates_deduped_function_tools() {
    assert!(
        deepseek_provider_core_dedup_and_validate_function_tools(
            vec![serde_json::json!({
                "function": {"name": "lookup", "strict": true}
            })],
            false,
            "DeepSeek",
        )
        .unwrap_err()
        .contains("DeepSeek strict function tool `lookup` requires deepseek.strict_tools=true")
    );

    assert!(
            deepseek_provider_core_dedup_and_validate_function_tools(
                vec![
                    serde_json::json!({"function": {"name": "lookup", "parameters": {"type": "object", "properties": {"a": {"type": "string"}}}}}),
                    serde_json::json!({"function": {"name": "lookup", "parameters": {"type": "object", "properties": {"b": {"type": "string"}}}}}),
                ],
                false,
                "DeepSeek",
            )
            .unwrap_err()
            .contains("DeepSeek function tool name `lookup` is duplicated after translation")
        );

    let tools = deepseek_provider_core_dedup_and_validate_function_tools(
            vec![serde_json::json!({
                "function": {"name": "lookup", "parameters": {"type": "object", "properties": {"query": {"type": "string"}}}}
            })],
            true,
            "DeepSeek",
        )
        .unwrap();
    assert_eq!(tools[0]["function"]["strict"], true);
}

#[test]
fn deepseek_provider_core_rejects_invalid_tool_choice() {
    assert!(
        deepseek_provider_core_validate_tool_choice_shape(
            &serde_json::json!({"tool_choice": "weird"}),
            false,
            "DeepSeek",
        )
        .unwrap_err()
        .contains("DeepSeek tool_choice string `weird` is not supported")
    );
    assert!(
        deepseek_provider_core_validate_tool_choice_shape(
            &serde_json::json!({"tool_choice": {"type": "function"}}),
            false,
            "DeepSeek",
        )
        .unwrap_err()
        .contains("DeepSeek named tool_choice requires a function name")
    );
    assert!(
        deepseek_provider_core_validate_tool_choice_shape(
            &serde_json::json!({"tool_choice": {"type": "file_search"}}),
            false,
            "DeepSeek",
        )
        .unwrap_err()
        .contains("DeepSeek tool_choice type `file_search` is not supported")
    );
    let tool_names = BTreeSet::new();
    assert!(
        deepseek_provider_core_validate_tool_choice_target(
            &serde_json::json!({"function": {"name": "missing"}}),
            &tool_names,
            "DeepSeek",
        )
        .unwrap_err()
        .contains("DeepSeek named tool_choice `missing` does not match")
    );
}

#[test]
fn deepseek_provider_core_validates_web_search_options() {
    deepseek_provider_core_validate_web_search_options(
        &serde_json::json!({
            "search_context_size": "medium",
            "allowed_domains": ["example.com"],
            "user_location": {}
        }),
        "DeepSeek",
    )
    .unwrap();

    assert!(
        deepseek_provider_core_validate_web_search_options(
            &serde_json::json!({"search_context_size": "huge"}),
            "DeepSeek",
        )
        .unwrap_err()
        .contains("DeepSeek web_search search_context_size must be low, medium, or high")
    );
    assert!(
        deepseek_provider_core_validate_web_search_options(
            &serde_json::json!({"allowed_domains": [""]}),
            "DeepSeek",
        )
        .unwrap_err()
        .contains("DeepSeek web_search allowed_domains entries must be non-empty strings")
    );
    assert!(
        deepseek_provider_core_validate_web_search_options(
            &serde_json::json!({"user_location": true}),
            "DeepSeek",
        )
        .unwrap_err()
        .contains("DeepSeek web_search user_location must be an object")
    );
}

#[test]
fn deepseek_provider_core_validates_web_search_tool_context_size() {
    deepseek_provider_core_validate_web_search_tool_context_size(
        &serde_json::json!({
            "tools": [
                {"type": "function", "function": {"name": "lookup"}},
                {"type": "web_search_preview", "context_size": "low"},
                {"type": "web_search_preview_2025_03_11", "search_context_size": "high"}
            ]
        }),
        "DeepSeek",
    )
    .unwrap();

    assert!(
        deepseek_provider_core_validate_web_search_tool_context_size(
            &serde_json::json!({"tools": [{"type": "web_search", "context_size": "huge"}]}),
            "DeepSeek",
        )
        .unwrap_err()
        .contains("DeepSeek web_search context_size must be low, medium, or high")
    );
    assert!(
        deepseek_provider_core_validate_web_search_tool_context_size(
            &serde_json::json!({"tools": [{"type": "web_search", "context_size": true}]}),
            "DeepSeek",
        )
        .unwrap_err()
        .contains("DeepSeek web_search context_size must be low, medium, or high")
    );
}

#[test]
fn deepseek_provider_core_validates_supported_input_items() {
    for item in [
        serde_json::json!({"type": "message", "role": "developer", "content": [{"type": "input_text", "text": "hi"}]}),
        serde_json::json!({"type": "function_call", "call_id": "call_1", "function": {"name": "lookup"}}),
        serde_json::json!({"type": "custom_tool_call_output", "call_id": "call_1", "output": "ok"}),
        serde_json::json!({"type": "local_shell_call", "call_id": "call_2", "action": {"command": ["echo", "hi"]}}),
        serde_json::json!({"type": "message", "content": [{"type": "input_image"}]}),
    ] {
        deepseek_provider_core_validate_supported_input_item(&item, true, "DeepSeek").unwrap();
    }
}

#[test]
fn deepseek_provider_core_rejects_unsupported_input_items() {
    assert!(
        deepseek_provider_core_validate_supported_input_item(
            &serde_json::json!(true),
            false,
            "DeepSeek",
        )
        .unwrap_err()
        .contains("DeepSeek input items must be objects")
    );
    assert!(
        deepseek_provider_core_validate_supported_input_item(
            &serde_json::json!({"type": "message", "role": "critic", "content": "no"}),
            false,
            "DeepSeek",
        )
        .unwrap_err()
        .contains("DeepSeek message role `critic` is not supported")
    );
    assert!(
        deepseek_provider_core_validate_supported_input_item(
            &serde_json::json!({"type": "function_call", "call_id": "call_1"}),
            false,
            "DeepSeek",
        )
        .unwrap_err()
        .contains("DeepSeek input tool call items require a function name")
    );
    assert!(
        deepseek_provider_core_validate_supported_input_item(
            &serde_json::json!({"type": "function_call_output", "call_id": "call_1"}),
            false,
            "DeepSeek",
        )
        .unwrap_err()
        .contains("DeepSeek input tool output items require output content")
    );
    assert!(
        deepseek_provider_core_validate_supported_input_item(
            &serde_json::json!({"type": "message", "content": [{"type": "input_image"}]}),
            false,
            "DeepSeek",
        )
        .unwrap_err()
        .contains(
            "DeepSeek text-only adapter does not support message content part type `input_image`"
        )
    );
    assert!(
        deepseek_provider_core_validate_supported_input_item(
            &serde_json::json!({"type": "message", "content": [{"type": "input_text"}]}),
            false,
            "DeepSeek",
        )
        .unwrap_err()
        .contains("DeepSeek input_text content parts require a text field")
    );
}

#[test]
fn deepseek_provider_core_extracts_history_replay_markers() {
    let history = vec![
        serde_json::json!({"role": "system", "content": "rules"}),
        serde_json::json!({
            "role": "assistant",
            "content": "already",
            "tool_calls": [
                {"id": "call_1"},
                {"id": ""}
            ]
        }),
        serde_json::json!({"role": "tool", "tool_call_id": "call_2", "content": "ok"}),
        serde_json::json!({"role": "developer", "content": "dev rules"}),
    ];

    assert!(deepseek_provider_core_history_has_system_message(
        &history, "rules"
    ));
    assert!(deepseek_provider_core_history_has_tool_call(
        &history, "call_1"
    ));
    assert_eq!(
        deepseek_provider_core_tool_call_ids(&history),
        BTreeSet::from(["call_1".to_string()])
    );
    assert_eq!(
        deepseek_provider_core_tool_output_call_ids(&history),
        BTreeSet::from(["call_2".to_string()])
    );
    assert_eq!(
        deepseek_provider_core_message_signatures(&history),
        BTreeSet::from([
            ("system".to_string(), "rules".to_string()),
            ("assistant".to_string(), "already".to_string()),
            ("system".to_string(), "dev rules".to_string()),
            ("tool".to_string(), "ok".to_string()),
        ])
    );

    assert_eq!(
        deepseek_provider_core_first_function_call_output_call_id(&serde_json::json!({
            "input": [
                {"type": "message", "content": "ignore"},
                {"type": "mcp_tool_result", "tool_call_id": "call_3", "content": "ok"}
            ]
        })),
        Some("call_3".to_string())
    );
}

#[test]
fn deepseek_provider_core_pushes_messages_from_response_items() {
    assert_eq!(deepseek_provider_core_chat_role("developer"), "system");
    assert_eq!(deepseek_provider_core_chat_role("critic"), "user");

    let mut messages = Vec::new();
    let replayed_tool_call_ids = BTreeSet::new();
    let replayed_tool_output_call_ids = BTreeSet::new();
    let replayed_message_signatures = BTreeSet::new();

    for item in [
        serde_json::json!({"type": "message", "role": "developer", "content": "rules"}),
        serde_json::json!({"type": "function_call", "call_id": "call_1", "name": "lookup", "arguments": "{\"q\":\"x\"}"}),
        serde_json::json!({"type": "custom_tool_call", "call_id": "call_2", "name": "apply_patch", "input": "*** Begin Patch\n*** End Patch"}),
        serde_json::json!({"type": "local_shell_call", "call_id": "call_3", "action": {"command": ["echo", "hi"], "timeout": 1}}),
        serde_json::json!({"type": "function_call_output", "call_id": "call_1", "output": "ok"}),
    ] {
        deepseek_provider_core_push_message_from_responses_item(
            &item,
            &mut messages,
            &replayed_tool_call_ids,
            &replayed_tool_output_call_ids,
            &replayed_message_signatures,
        );
    }

    assert_eq!(
        messages[0],
        serde_json::json!({"role": "system", "content": "rules"})
    );
    assert_eq!(messages[1]["tool_calls"][0]["function"]["name"], "lookup");
    assert_eq!(
        messages[2]["tool_calls"][0]["function"]["name"],
        "apply_patch"
    );
    assert_eq!(
        messages[3]["tool_calls"][0]["function"]["name"],
        "shell_command"
    );
    assert_eq!(
        messages[4],
        serde_json::json!({"role": "tool", "tool_call_id": "call_1", "content": "ok"})
    );
}

#[test]
fn deepseek_provider_core_skips_replayed_response_items() {
    let mut messages = Vec::new();
    let replayed_tool_call_ids = BTreeSet::from(["call_1".to_string()]);
    let replayed_tool_output_call_ids = BTreeSet::from(["call_2".to_string()]);
    let replayed_message_signatures = BTreeSet::from([("user".to_string(), "already".to_string())]);

    for item in [
        serde_json::json!({"type": "message", "role": "user", "content": "already"}),
        serde_json::json!({"type": "function_call", "call_id": "call_1", "name": "lookup"}),
        serde_json::json!({"type": "function_call_output", "call_id": "call_2", "output": "ok"}),
        serde_json::json!({"type": "mcp_call", "call_id": "call_3", "name": "mcp_lookup", "result": {"ok": true}}),
    ] {
        deepseek_provider_core_push_message_from_responses_item(
            &item,
            &mut messages,
            &replayed_tool_call_ids,
            &replayed_tool_output_call_ids,
            &replayed_message_signatures,
        );
    }

    assert_eq!(messages.len(), 2);
    assert_eq!(
        messages[0]["tool_calls"][0]["function"]["name"],
        "mcp_lookup"
    );
    assert_eq!(messages[1]["role"], "tool");
    assert_eq!(messages[1]["tool_call_id"], "call_3");
}
