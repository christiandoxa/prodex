//! Kiro provider-core characterization tests.

use super::*;
use serde_json::{Value, json};

#[test]
fn kiro_provider_core_translates_chat_controls_and_messages() {
    let translated = kiro_provider_core_chat_completions_request_body(
        serde_json::to_vec(&json!({
            "model": "claude-sonnet-4",
            "messages": [{"role": "user", "content": "hello"}],
            "temperature": 1,
            "parallel_tool_calls": true,
            "max_tokens": 32,
            "user": "user-123"
        }))
        .unwrap()
        .as_slice(),
    )
    .unwrap();
    let translated: Value = serde_json::from_slice(&translated).unwrap();
    assert!(translated.get("temperature").is_none());
    assert!(translated.get("parallel_tool_calls").is_none());
    assert!(translated.get("max_tokens").is_none());
    assert!(translated.get("user").is_none());
    assert_eq!(translated["input"][0]["role"], "user");
}

#[test]
fn kiro_provider_core_builds_prompt_from_chat_messages() {
    let prompt = kiro_provider_core_prompt_from_chat_messages(&[
        json!({"role": "system", "content": "rules"}),
        json!({"role": "assistant", "content": [{"text": "plan"}]}),
        json!({
            "role": "assistant",
            "tool_calls": [{
                "function": {"name": "run_shell", "arguments": "{\"cmd\":\"pwd\"}"}
            }]
        }),
        json!({"role": "tool", "content": {"output": "done"}}),
        json!({"role": "user", "content": "   "}),
    ]);

    assert_eq!(
        prompt,
        "System:\nrules\n\nAssistant:\nplan\n\nAssistant:\nTool call run_shell: {\"cmd\":\"pwd\"}\n\nTool:\ndone"
    );
    assert_eq!(
        kiro_provider_core_prompt_from_chat_messages(&[json!({"role": "user"})]),
        "User:\n"
    );
}

#[test]
fn kiro_provider_core_shapes_acp_model_value() {
    assert_eq!(
        kiro_provider_core_acp_model_value("claude-sonnet-4", "Claude Sonnet 4"),
        json!({
            "id": "claude-sonnet-4",
            "name": "Claude Sonnet 4",
            "object": "model",
            "owned_by": "kiro-cli",
        })
    );
}

#[test]
fn kiro_provider_core_shapes_acp_requests() {
    assert_eq!(
        kiro_provider_core_acp_initialize_request(1, "prodex", "Prodex", "0.1.0"),
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": 1,
                "clientCapabilities": {
                    "fs": {
                        "readTextFile": false,
                        "writeTextFile": false,
                    },
                    "terminal": false,
                    "auth": {
                        "terminal": false,
                    },
                },
                "clientInfo": {
                    "name": "prodex",
                    "title": "Prodex",
                    "version": "0.1.0",
                },
            }
        })
    );
    assert_eq!(
        kiro_provider_core_acp_session_new_request(2, "/tmp/prodex"),
        json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "session/new",
            "params": {
                "cwd": "/tmp/prodex",
                "mcpServers": [],
            }
        })
    );
    assert_eq!(
        kiro_provider_core_acp_session_prompt_request(3, "sess_1", "hello"),
        json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "session/prompt",
            "params": {
                "sessionId": "sess_1",
                "prompt": [{"type": "text", "text": "hello"}],
            }
        })
    );
}

#[test]
fn kiro_provider_core_shapes_model_endpoint_values() {
    let catalog = vec![json!({
        "id": "claude-sonnet-4",
        "object": "model",
        "owned_by": "kiro-cli",
    })];

    assert_eq!(
        kiro_provider_core_model_list_value(&catalog),
        json!({
            "object": "list",
            "data": catalog,
        })
    );

    let (status, body) = kiro_provider_core_model_value_or_not_found(&catalog, "CLAUDE-SONNET-4");
    assert_eq!(status, 200);
    assert_eq!(body["id"], "claude-sonnet-4");

    let (status, body) = kiro_provider_core_model_value_or_not_found(&catalog, "missing");
    assert_eq!(status, 404);
    assert_eq!(body["error"]["code"], "model_not_found");
    assert_eq!(
        body["error"]["message"],
        "model 'missing' is not available for kiro"
    );
}

#[test]
fn kiro_provider_core_shapes_error_values() {
    assert_eq!(
        kiro_provider_core_invalid_request_error_value("bad request", "bad_request"),
        json!({
            "error": {
                "message": "bad request",
                "type": "invalid_request_error",
                "code": "bad_request",
            }
        })
    );
    assert_eq!(
        kiro_provider_core_unsupported_path_error_value("/v1/files"),
        json!({
            "error": {
                "message": "Kiro provider does not support /v1/files yet",
                "type": "invalid_request_error",
                "code": "unsupported_path",
            }
        })
    );
}

#[test]
fn kiro_provider_core_applies_response_runtime_metadata() {
    let mut response = json!({"id": "resp_1"});
    kiro_provider_core_apply_response_runtime_metadata(
        &mut response,
        "kiro-main",
        Some("claude-sonnet-4"),
        Some(123),
    );

    assert_eq!(response["created_at"], 123);
    assert_eq!(response["metadata"]["kiro"]["profile_name"], "kiro-main");
    assert_eq!(response["requested_model"], "claude-sonnet-4");

    kiro_provider_core_apply_response_runtime_metadata(&mut response, "kiro-main", Some(""), None);
    assert_eq!(response["created_at"], 123);
    assert_eq!(response["requested_model"], "claude-sonnet-4");
}

#[test]
fn kiro_provider_core_shapes_acp_assistant_output_message() {
    assert_eq!(
        kiro_provider_core_acp_assistant_output_message("hello"),
        json!({
            "type": "message",
            "role": "assistant",
            "content": [{
                "type": "output_text",
                "text": "hello",
            }],
        })
    );
}

#[test]
fn kiro_provider_core_shapes_acp_response_value() {
    assert_eq!(
        kiro_provider_core_acp_response_value(
            "resp_1",
            123,
            "claude-sonnet-4",
            vec![json!({"type": "message"})],
        ),
        json!({
            "id": "resp_1",
            "object": "response",
            "created_at": 123,
            "model": "claude-sonnet-4",
            "output": [{"type": "message"}],
        })
    );
}

#[test]
fn kiro_provider_core_shapes_acp_chat_assistant_message() {
    assert_eq!(
        kiro_provider_core_acp_chat_assistant_message("", "", Vec::new()),
        None
    );
    assert_eq!(
        kiro_provider_core_acp_chat_assistant_message("", "thoughts", Vec::new()).unwrap(),
        json!({
            "role": "assistant",
            "content": null,
            "reasoning_content": "thoughts",
        })
    );
    assert_eq!(
        kiro_provider_core_acp_chat_assistant_message(
            "",
            "",
            vec![json!({"id": "call_1", "type": "function"})],
        )
        .unwrap(),
        json!({
            "role": "assistant",
            "content": "",
            "tool_calls": [{"id": "call_1", "type": "function"}],
        })
    );
}

#[test]
fn kiro_provider_core_shapes_acp_plan_entry() {
    assert_eq!(
        kiro_provider_core_acp_plan_entry("read files", "high", "pending"),
        json!({
            "content": "read files",
            "priority": "high",
            "status": "pending",
        })
    );
}

#[test]
fn kiro_provider_core_shapes_acp_error_value() {
    assert_eq!(
        kiro_provider_core_acp_error_value(-32000, "boom"),
        json!({
            "code": "-32000",
            "message": "boom",
        })
    );
}

#[test]
fn kiro_provider_core_marks_acp_failed_response() {
    let mut response = kiro_provider_core_acp_response_value("resp_1", 1, "kiro", Vec::new());
    kiro_provider_core_acp_mark_failed_response(&mut response, -32000, "boom");

    assert_eq!(response["status"], "failed");
    assert_eq!(
        response["error"],
        json!({"code": "-32000", "message": "boom"})
    );
}

#[test]
fn kiro_provider_core_shapes_acp_session_info() {
    assert_eq!(
        kiro_provider_core_acp_session_info(Some("Session"), None),
        json!({
            "title": "Session",
            "updated_at": null,
        })
    );
}

#[test]
fn kiro_provider_core_shapes_acp_metadata() {
    assert_eq!(
        kiro_provider_core_acp_metadata("", None, None, None, None, None, None, None),
        None
    );
    assert_eq!(
        kiro_provider_core_acp_metadata(
            "thoughts",
            Some(json!({"used": 3})),
            Some(vec![json!({"content": "plan"})]),
            Some(vec![json!({"name": "cmd"})]),
            Some("agent"),
            Some("Session"),
            Some("2026-07-08T00:00:00Z"),
            Some("end_turn"),
        )
        .unwrap(),
        json!({
            "kiro": {
                "reasoning_content": "thoughts",
                "usage_update": {"used": 3},
                "plan": [{"content": "plan"}],
                "available_commands": [{"name": "cmd"}],
                "current_mode_id": "agent",
                "session_info": {
                    "title": "Session",
                    "updated_at": "2026-07-08T00:00:00Z",
                },
                "stop_reason": "end_turn",
            }
        })
    );
}

#[test]
fn kiro_provider_core_maps_acp_incomplete_details() {
    assert_eq!(
        kiro_provider_core_acp_incomplete_details(Some("max_tokens")),
        Some((
            "max_output_tokens",
            "Kiro stopped before end_turn because the model hit its output limit."
        ))
    );
    assert_eq!(
        kiro_provider_core_acp_incomplete_details(Some("max_turn_requests")),
        Some((
            "max_turn_requests",
            "Kiro stopped before end_turn because the turn hit its request limit."
        ))
    );
    assert_eq!(
        kiro_provider_core_acp_incomplete_details(Some("unknown")),
        None
    );
}

#[test]
fn kiro_provider_core_shapes_acp_incomplete_details_value() {
    assert_eq!(
        kiro_provider_core_acp_incomplete_details_value("max_output_tokens", "hit limit"),
        json!({
            "reason": "max_output_tokens",
            "message": "hit limit",
        })
    );
}

#[test]
fn kiro_provider_core_marks_acp_incomplete_response() {
    let mut response = kiro_provider_core_acp_response_value("resp_1", 1, "kiro", Vec::new());
    kiro_provider_core_acp_mark_incomplete_response(
        &mut response,
        "max_output_tokens",
        "hit limit",
    );

    assert_eq!(response["status"], "incomplete");
    assert_eq!(
        response["incomplete_details"],
        json!({"reason": "max_output_tokens", "message": "hit limit"})
    );
}

#[test]
fn kiro_provider_core_extracts_acp_stop_reason() {
    assert_eq!(
        kiro_provider_core_acp_stop_reason(Some(&json!({"stopReason": "max_tokens"}))).as_deref(),
        Some("max_tokens")
    );
    assert_eq!(
        kiro_provider_core_acp_stop_reason(Some(&json!({"stop_reason": "refusal"}))).as_deref(),
        Some("refusal")
    );
    assert_eq!(
        kiro_provider_core_acp_stop_reason(Some(&json!({"status": "cancelled"}))).as_deref(),
        Some("cancelled")
    );
    assert_eq!(
        kiro_provider_core_acp_stop_reason(Some(&json!({"stopReason": 7}))),
        None
    );
    assert_eq!(kiro_provider_core_acp_stop_reason(None), None);
}

#[test]
fn kiro_provider_core_rejects_parallel_tool_control() {
    let error = kiro_provider_core_chat_completions_request_body(
        serde_json::to_vec(&json!({
            "model": "claude-sonnet-4",
            "messages": [{"role": "user", "content": "hello"}],
            "parallel_tool_calls": false
        }))
        .unwrap()
        .as_slice(),
    )
    .unwrap_err();
    assert_eq!(error.code, "unsupported_parallel_tool_calls");
}

#[test]
fn kiro_provider_core_maps_responses_value_to_chat_completion() {
    let completion = kiro_provider_core_chat_completion_value_from_response(
        &json!({
            "id": "resp_kiro_1",
            "created_at": 123,
            "model": "claude-sonnet-4",
            "output": [
                {
                    "type": "message",
                    "content": [{"type": "output_text", "text": "hello"}]
                },
                {
                    "type": "function_call",
                    "call_id": "call_1",
                    "name": "run",
                    "arguments": "{}"
                }
            ],
            "metadata": {"kiro": {"reasoning_content": "thoughts"}}
        }),
        7,
    );

    assert_eq!(completion["id"], "chatcmpl_resp_kiro_1");
    assert_eq!(completion["choices"][0]["message"]["content"], "hello");
    assert_eq!(
        completion["choices"][0]["message"]["tool_calls"][0]["function"]["name"],
        "run"
    );
    assert_eq!(completion["choices"][0]["finish_reason"], "tool_calls");
    assert_eq!(
        completion["choices"][0]["message"]["reasoning_content"],
        "thoughts"
    );
}

#[test]
fn kiro_provider_core_detects_response_tool_calls() {
    let tool_response = json!({
        "output": [
            {"type": "message"},
            {"type": "function_call", "call_id": "call_1"},
        ]
    });
    assert!(kiro_provider_core_response_has_tool_calls(&tool_response));
    assert_eq!(
        kiro_provider_core_chat_completion_finish_reason_from_response(&tool_response),
        "tool_calls"
    );

    let length_response = json!({
        "output": [{"type": "message"}],
        "incomplete_details": {"reason": "max_output_tokens"},
    });
    assert_eq!(
        kiro_provider_core_chat_completion_finish_reason_from_response(&length_response),
        "length"
    );
    assert!(!kiro_provider_core_response_has_tool_calls(&json!({
        "output": [{"type": "message"}]
    })));
    assert_eq!(
        kiro_provider_core_chat_completion_finish_reason_from_response(&json!({})),
        "stop"
    );
    assert!(!kiro_provider_core_response_has_tool_calls(&json!({})));
}

#[test]
fn kiro_provider_core_maps_responses_value_to_anthropic_message() {
    let message = kiro_provider_core_anthropic_message_value_from_response(
        &json!({
            "id": "resp_kiro_1",
            "output": [
                {
                    "type": "message",
                    "content": [{"type": "output_text", "text": "hello"}]
                },
                {
                    "type": "function_call",
                    "call_id": "call_1",
                    "name": "run_shell",
                    "arguments": "{\"cmd\":\"pwd\"}"
                }
            ],
            "usage": {"input_tokens": 3, "output_tokens": 5},
            "metadata": {"kiro": {"stop_reason": "max_output_tokens"}}
        }),
        "claude-sonnet-4",
    );

    assert_eq!(message["id"], "resp_kiro_1");
    assert_eq!(message["type"], "message");
    assert_eq!(message["role"], "assistant");
    assert_eq!(message["model"], "claude-sonnet-4");
    assert_eq!(message["stop_reason"], "tool_use");
    assert_eq!(message["stop_sequence"], Value::Null);
    assert_eq!(message["usage"]["input_tokens"], 3);
    assert_eq!(message["usage"]["output_tokens"], 5);
    assert_eq!(
        message["content"],
        json!([
            {
                "type": "tool_use",
                "id": "call_1",
                "name": "run_shell",
                "input": {"cmd": "pwd"},
            },
            {
                "type": "text",
                "text": "hello",
            }
        ])
    );

    let message = kiro_provider_core_anthropic_message_value_from_response(
        &json!({
            "output": [{"type": "message", "content": [{"text": "limit"}]}],
            "metadata": {"kiro": {"stop_reason": "max_output_tokens"}}
        }),
        "kiro",
    );
    assert_eq!(message["id"], "msg_kiro");
    assert_eq!(message["stop_reason"], "max_tokens");
    assert_eq!(message["usage"]["input_tokens"], 0);
    assert_eq!(message["usage"]["output_tokens"], 0);
}

#[test]
fn kiro_provider_core_builds_chat_completion_stream_chunk() {
    let chunk = kiro_provider_core_chat_completion_chunk(
        "chatcmpl_1",
        Some("claude-sonnet-4"),
        json!({"role": "assistant", "content": "hello"}),
        None,
    )
    .unwrap();
    let chunk = String::from_utf8(chunk).unwrap();
    assert!(chunk.starts_with("data: "));
    assert!(chunk.ends_with("\n\n"));
    assert!(chunk.contains("\"object\":\"chat.completion.chunk\""));
    assert!(chunk.contains("\"model\":\"claude-sonnet-4\""));
    assert!(chunk.contains("\"content\":\"hello\""));
}

#[test]
fn kiro_provider_core_shapes_chat_completion_stream_deltas() {
    assert_eq!(
        kiro_provider_core_chat_completion_role_delta(),
        json!({"role": "assistant"})
    );
    assert_eq!(kiro_provider_core_chat_completion_empty_delta(), json!({}));
    assert_eq!(
        kiro_provider_core_chat_completion_text_delta("hello", true),
        json!({"role": "assistant", "content": "hello"})
    );
    assert_eq!(
        kiro_provider_core_chat_completion_text_delta("again", false),
        json!({"content": "again"})
    );
    assert_eq!(
        kiro_provider_core_chat_completion_tool_call_delta("call_1", "run_shell", "{}", true),
        json!({
            "role": "assistant",
            "tool_calls": [{
                "index": 0,
                "id": "call_1",
                "type": "function",
                "function": {
                    "name": "run_shell",
                    "arguments": "{}",
                }
            }]
        })
    );
    assert_eq!(
        kiro_provider_core_chat_completion_tool_call_delta("call_1", "run_shell", "{}", false),
        json!({
            "tool_calls": [{
                "index": 0,
                "id": "call_1",
                "type": "function",
                "function": {
                    "name": "run_shell",
                    "arguments": "{}",
                }
            }]
        })
    );
}

#[test]
fn kiro_provider_core_shapes_response_stream_events() {
    let item = json!({"type": "function_call", "call_id": "call_1"});
    let response = json!({"id": "resp_1"});

    assert_eq!(
        kiro_provider_core_output_text_delta_event(2, 123, "resp_1", "hello"),
        json!({
            "type": "response.output_text.delta",
            "sequence_number": 2,
            "created_at": 123,
            "response_id": "resp_1",
            "delta": "hello",
        })
    );
    assert_eq!(
        kiro_provider_core_response_created_event(1, 123, "resp_1"),
        json!({
            "type": "response.created",
            "sequence_number": 1,
            "created_at": 123,
            "response": {"id": "resp_1"},
        })
    );
    assert_eq!(
        kiro_provider_core_output_item_added_event(3, &item),
        json!({
            "type": "response.output_item.added",
            "sequence_number": 3,
            "item": item,
        })
    );
    assert_eq!(
        kiro_provider_core_output_item_done_event(4, "resp_1", &item),
        json!({
            "type": "response.output_item.done",
            "sequence_number": 4,
            "item": item,
            "response_id": "resp_1",
        })
    );
    assert_eq!(
        kiro_provider_core_response_completed_event(5, 123, &response),
        json!({
            "type": "response.completed",
            "sequence_number": 5,
            "created_at": 123,
            "response": response,
        })
    );
    assert_eq!(
        kiro_provider_core_tool_call_arguments_delta_chat_value("call_1", "{\"cmd\":\"pwd\"}"),
        json!({
            "choices": [{
                "delta": {
                    "tool_calls": [{
                        "id": "call_1",
                        "function": {
                            "arguments": "{\"cmd\":\"pwd\"}",
                        }
                    }]
                }
            }]
        })
    );
}

#[test]
fn kiro_provider_core_extracts_stream_content_text() {
    assert_eq!(
        kiro_provider_core_stream_content_text(&json!([
            {"text": "hello "},
            {"content": [{"text": "kiro"}]},
            {"text": ""}
        ]))
        .as_deref(),
        Some("hello kiro")
    );
    assert_eq!(
        kiro_provider_core_stream_content_text(&json!({"content": []})),
        None
    );
}

#[test]
fn kiro_provider_core_shapes_stream_tool_call_item() {
    assert_eq!(
        kiro_provider_core_stream_tool_arguments(Some(&json!({"cmd": "pwd"}))),
        "{\"cmd\":\"pwd\"}"
    );
    assert_eq!(kiro_provider_core_stream_tool_arguments(None), "{}");

    let item = kiro_provider_core_stream_tool_call_item(
        "call_1",
        Some("Run Shell!"),
        Some("completed"),
        Some("command"),
        Some(&json!({"cmd": "pwd"})),
    );

    assert_eq!(item["type"], "function_call");
    assert_eq!(item["call_id"], "call_1");
    assert_eq!(item["name"], "run_shell");
    assert_eq!(item["namespace"], "kiro");
    assert_eq!(item["arguments"], "{\"cmd\":\"pwd\"}");
    assert_eq!(item["metadata"]["kiro"]["title"], "Run Shell!");
    assert_eq!(item["metadata"]["kiro"]["status"], "completed");
    assert_eq!(item["metadata"]["kiro"]["kind"], "command");
}

#[test]
fn kiro_provider_core_shapes_acp_tool_call_items() {
    let raw_input = json!({"cmd": "pwd"});
    let raw_output = json!({"exit_code": 0});
    let content = vec![json!({"text": "done"})];
    let locations = vec![json!({"path": "/tmp"})];

    let response_item = kiro_provider_core_acp_responses_tool_call_item(
        "call_1",
        Some("Run Shell!"),
        Some("completed"),
        Some("command"),
        Some(&raw_input),
        Some(&raw_output),
        Some(&content),
        Some(&locations),
    );
    assert_eq!(response_item["type"], "function_call");
    assert_eq!(response_item["call_id"], "call_1");
    assert_eq!(response_item["name"], "run_shell");
    assert_eq!(response_item["arguments"], "{\"cmd\":\"pwd\"}");
    assert_eq!(response_item["metadata"]["kiro"]["raw_output"], raw_output);
    assert_eq!(response_item["metadata"]["kiro"]["content"], json!(content));
    assert_eq!(
        response_item["metadata"]["kiro"]["locations"],
        json!(locations)
    );

    let chat_item = kiro_provider_core_acp_chat_tool_call_item(
        "call_1",
        Some("Run Shell!"),
        Some("command"),
        Some(&raw_input),
    );
    assert_eq!(chat_item["id"], "call_1");
    assert_eq!(chat_item["type"], "function");
    assert_eq!(chat_item["function"]["name"], "run_shell");
    assert_eq!(chat_item["function"]["arguments"], "{\"cmd\":\"pwd\"}");
}

#[test]
fn kiro_provider_core_shapes_acp_usage_update() {
    let usage = kiro_provider_core_acp_usage_update_json(12, 10, Some((0.25, "USD")));

    assert_eq!(usage["used"], 12);
    assert_eq!(usage["size"], 10);
    assert_eq!(usage["remaining"], 0);
    assert_eq!(usage["cost"]["amount"], 0.25);
    assert_eq!(usage["cost"]["currency"], "USD");
}

#[test]
fn kiro_provider_core_rewrites_semantic_compact_request() {
    let body = serde_json::to_vec(&json!({
        "model": "claude-sonnet-4",
        "stream": true,
        "store": true,
        "previous_response_id": "resp_old",
        "tools": [{"type": "function", "name": "shell"}],
        "tool_choice": "auto",
        "text": {"format": {"type": "json_schema"}},
        "input": [{"type": "message", "role": "user", "content": "summarize"}]
    }))
    .unwrap();

    let rewritten = kiro_provider_core_semantic_compact_request_body(&body).unwrap();
    let value: Value = serde_json::from_slice(&rewritten).unwrap();

    assert_eq!(value["stream"], false);
    assert_eq!(value["store"], false);
    assert!(value.get("previous_response_id").is_none());
    assert!(value.get("tools").is_none());
    assert!(value.get("tool_choice").is_none());
    assert!(value.get("text").is_none());
    assert_eq!(value["input"].as_array().unwrap().len(), 2);
    assert!(
        value["input"][1]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("durable continuation summary")
    );
}

#[test]
fn kiro_provider_core_extracts_compact_summary_from_response() {
    let response = json!({
        "output": [{
            "type": "message",
            "content": [{"type": "output_text", "text": " Summary text. "}]
        }]
    });

    assert_eq!(
        kiro_provider_core_compact_summary_from_response(&response).unwrap(),
        "Summary text."
    );
    assert_eq!(
        kiro_provider_core_compact_summary_from_response(&json!({"output": []})).unwrap_err(),
        "Kiro compact response returned no summary text"
    );
}
