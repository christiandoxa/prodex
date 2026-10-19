use super::super::transcript_events_from_session_line;
use serde_json::{Value, json};

fn show(record_type: &str, event_type: &str, mut payload: Value) -> Vec<(String, String)> {
    payload
        .as_object_mut()
        .unwrap()
        .insert("type".to_string(), json!(event_type));
    transcript_events_from_session_line(
        &json!({"type": record_type, "payload": payload}).to_string(),
    )
    .into_iter()
    .map(|event| (event.source, event.text))
    .collect()
}

#[test]
fn operation_projection_preserves_alias_priority_and_inferred_subagent_status() {
    assert_eq!(
        show(
            "event_msg",
            "sub_agent_completed",
            json!({
                "server": 42, "server_label": "\u{2003}", "server_name": "fallback",
                "tool": null, "tool_name": "shell", "name": "", "status": " ", "phase": "running",
            })
        ),
        vec![(
            "agent".to_string(),
            "server=fallback tool=shell phase=running status=completed".to_string()
        )]
    );
    assert_eq!(
        show(
            "event_msg",
            "subagent_failed",
            json!({"status": "explicit"})
        ),
        vec![("agent".to_string(), "status=explicit".to_string())]
    );
    assert_eq!(
        show("event_msg", "subagent_", json!({})),
        vec![("agent".to_string(), "status=".to_string())]
    );
    for record in ["event_msg", "response_item"] {
        assert_eq!(
            show(record, "mcp_tool_call_begin", json!({})),
            vec![("mcp".to_string(), "mcp tool call begin".to_string())]
        );
    }
}

#[test]
fn operation_projection_preserves_scalar_types_json_spelling_and_output_order() {
    assert_eq!(show("event_msg", "command_execution_completed", json!({
        "status": "failed", "exit_code": -7, "exit_status": false, "reason": ["ignored"],
        "duration_ms": 1.5, "message": "Bearer FAKE", "stdout": {"b": true, "a": "ok"},
        "stderr": ["one", 2], "output": "line\0tail",
    })), vec![("error".to_string(), "status=failed exit_code=-7 exit_status=false duration_ms=1.5 message=Bearer <redacted> stdout:\n{\"a\":\"ok\",\"b\":true} stderr:\n[\"one\",2]".to_string())]);
    assert_eq!(
        show(
            "event_msg",
            "task_complete",
            json!({"status": false, "stdout": []})
        ),
        vec![(
            "terminal".to_string(),
            "status=false stdout:\n[]".to_string()
        )]
    );
    assert_eq!(
        show(
            "event_msg",
            "exec_command_end",
            json!({"message": null, "stderr": "\u{3000}"})
        ),
        vec![("terminal".to_string(), "exec command end".to_string())]
    );
}

#[test]
fn operation_projection_keeps_redacted_protocol_values_and_existing_unicode_bound() {
    let long = "東京🧭".repeat(512);
    let redacted = redaction::redaction_redact_secret_like_text(&long);
    let (prefix, truncated) = prodex_mojo_core::log::transcript_operation_span(&redacted)
        .unwrap()
        .unwrap();
    assert!(truncated);
    let expected = format!("name={prefix}… status=Bearer <redacted>");
    assert_eq!(
        show(
            "event_msg",
            "mcp_tool_call",
            json!({"name": long, "status": "Bearer FAKE"})
        ),
        vec![("mcp".to_string(), expected)]
    );
}

#[test]
fn operation_projection_keeps_large_container_output_and_final_text_bound() {
    let output = json!({"authorization": "Bearer FAKE", "text": "東京🧭\0\n".repeat(8192)});
    let native = redaction::redaction_redact_secret_like_text(&output.to_string());
    let native = prodex_mojo_core::log::normalize_transcript_safe_text(&native).unwrap();
    let expected =
        prodex_mojo_core::log::normalize_transcript_safe_text(&format!("stdout:\n{native}"))
            .unwrap();
    assert_eq!(
        show("event_msg", "task_complete", json!({"stdout": output})),
        vec![("terminal".to_string(), expected.clone())]
    );
    assert!(expected.len() <= 65_536 && expected.ends_with(" …[truncated]"));
    assert!(!expected.contains("FAKE"));
}
