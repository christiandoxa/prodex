//! Kiro ACP provider-core characterization tests.

use super::*;
use serde_json::json;

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
        kiro_provider_core_acp_metadata("", None, None, None, None, None, None, None, Vec::new()),
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
            Vec::new(),
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
fn kiro_provider_core_acp_shapes_escape_unicode_and_controls() {
    assert_eq!(
        kiro_provider_core_acp_session_prompt_request(9, "会话\n\"1", "héllo\t界\\"),
        json!({
            "jsonrpc": "2.0",
            "id": 9,
            "method": "session/prompt",
            "params": {
                "sessionId": "会话\n\"1",
                "prompt": [{"type": "text", "text": "héllo\t界\\"}],
            }
        })
    );
    assert_eq!(
        kiro_provider_core_acp_error_value(i64::MIN, "échec\n界"),
        json!({"code": i64::MIN.to_string(), "message": "échec\n界"})
    );
}
