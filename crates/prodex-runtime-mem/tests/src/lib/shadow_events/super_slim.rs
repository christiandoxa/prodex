use super::*;

#[test]
fn super_slim_shadow_user_prompt_stores_summary_counts_and_ref_not_full_prompt() {
    let prompt = "\n\nImplement shadow transcript helpers\n".to_string() + &"detail ".repeat(400);
    let event = serde_json::json!({
        "payload": {
            "type": "user_message",
            "message": prompt,
            "metadata": {
                "artifact_ref": "prodex-artifact:prompt-123"
            }
        }
    });

    let shadow = runtime_mem_super_slim_shadow_codex_event(&event);
    let summary = lookup_test_path(&shadow, "payload.prompt_summary")
        .and_then(Value::as_str)
        .expect("shadow prompt summary should exist");
    let shadow_message = lookup_test_path(&shadow, "payload.message")
        .and_then(Value::as_str)
        .expect("shadow prompt body should exist");

    assert!(summary.starts_with("u: Implement shadow transcript helpers"));
    assert!(summary.contains("b="));
    assert!(summary.contains("t~="));
    assert!(summary.contains("ref=prodex-artifact:prompt-123"));
    assert!(summary.contains("omit=prompt"));
    assert!(!summary.contains("; t~="));
    assert!(!summary.contains("; ref="));
    assert!(!summary.contains("tok~="));
    assert!(!summary.contains("prompt omitted"));
    assert_eq!(shadow_message, "ss:omit");
    assert_ne!(
        lookup_test_path(&event, "payload.message").and_then(Value::as_str),
        Some(shadow_message)
    );
    assert_eq!(
        resolve_schema_user_prompt(&runtime_mem_super_slim_codex_schema(), &shadow).as_deref(),
        Some(summary)
    );
}

#[test]
fn super_slim_shadow_codex_129_response_messages_omit_raw_content_text() {
    let prompt = "Implement Codex 0.129 transcript compatibility\n".to_string()
        + &"large prompt detail ".repeat(300);
    let user_event = serde_json::json!({
        "type": "response_item",
        "payload": {
            "type": "message",
            "role": "user",
            "content": [
                { "type": "input_text", "text": prompt },
                { "type": "input_text", "text": "secondary prompt chunk" }
            ],
            "metadata": {
                "artifact_ref": "prodex-artifact:prompt-129"
            }
        }
    });
    let assistant_event = serde_json::json!({
        "type": "response_item",
        "payload": {
            "type": "message",
            "role": "assistant",
            "content": [
                { "type": "output_text", "text": "Completed Codex 0.129 support\n".to_string() + &"assistant detail ".repeat(300) }
            ]
        }
    });
    let shell_event = serde_json::json!({
        "type": "response_item",
        "payload": {
            "type": "local_shell_call",
            "call_id": "call-129",
            "action": { "command": "cargo test -q -p prodex-runtime-mem" }
        }
    });

    let user_shadow = runtime_mem_super_slim_shadow_codex_event(&user_event);
    let assistant_shadow = runtime_mem_super_slim_shadow_codex_event(&assistant_event);
    assert_eq!(
        lookup_test_path(&user_shadow, "payload.content[0].text").and_then(Value::as_str),
        Some("ss:omit")
    );
    assert_eq!(
        lookup_test_path(&assistant_shadow, "payload.content[0].text").and_then(Value::as_str),
        Some("ss:omit")
    );
    let user_summary = lookup_test_path(&user_shadow, "payload.prompt_summary")
        .and_then(Value::as_str)
        .expect("Codex 0.129 user message should have prompt summary");
    let assistant_summary = lookup_test_path(&assistant_shadow, "payload.summary")
        .and_then(Value::as_str)
        .expect("Codex 0.129 assistant message should have summary");
    assert!(user_summary.starts_with("u: Implement Codex 0.129 transcript compatibility"));
    assert!(assistant_summary.starts_with("a: Completed Codex 0.129 support"));

    let shadows = runtime_mem_super_slim_v2_shadow_codex_events([
        &user_event,
        &assistant_event,
        &shell_event,
    ]);
    assert_eq!(shadows[0]["t"].as_str(), Some("pm2:u"));
    assert_eq!(shadows[0]["r"].as_str(), Some("prodex-artifact:prompt-129"));
    assert_eq!(shadows[1]["t"].as_str(), Some("pm2:a"));
    assert_eq!(shadows[2]["t"].as_str(), Some("pm2:tu"));
    assert_eq!(
        shadows[2]["c"].as_str(),
        Some("cargo test -q -p prodex-runtime-mem")
    );
}

#[test]
fn super_slim_shadow_assistant_message_uses_short_summary() {
    let message =
        "Completed helper implementation\n".to_string() + &"verbose explanation ".repeat(300);
    let shadow = runtime_mem_super_slim_shadow_codex_event(&serde_json::json!({
        "payload": {
            "type": "agent_message",
            "message": message
        }
    }));
    let summary = lookup_test_path(&shadow, "payload.summary")
        .and_then(Value::as_str)
        .expect("assistant summary should exist");

    assert!(summary.starts_with("a: Completed helper implementation"));
    assert!(summary.contains("b="));
    assert!(summary.contains("t~="));
    assert!(summary.contains("omit=message"));
    assert!(!summary.contains("; t~="));
    assert_eq!(
        lookup_test_path(&shadow, "payload.message").and_then(Value::as_str),
        Some("ss:omit")
    );
    assert_eq!(
        resolve_schema_assistant_message(&runtime_mem_super_slim_codex_schema(), &shadow)
            .as_deref(),
        Some(summary)
    );
}

#[test]
fn super_slim_shadow_referenced_artifact_uses_shorter_prefix_than_plain_summary() {
    let tail = "TAIL_AFTER_SHORT_REF_CAP";
    let line = format!(
        "{}{tail}",
        "x".repeat(RUNTIME_MEM_SUPER_SLIM_REFERENCED_SUMMARY_PREFIX_CHAR_LIMIT + 8)
    );
    assert!(line.chars().count() < RUNTIME_MEM_SUPER_SLIM_SUMMARY_PREFIX_CHAR_LIMIT);

    let plain_shadow = runtime_mem_super_slim_shadow_codex_event(&serde_json::json!({
        "payload": {
            "type": "function_call_output",
            "call_id": "plain-call",
            "output": line
        }
    }));
    let artifact_shadow = runtime_mem_super_slim_shadow_codex_event(&serde_json::json!({
        "payload": {
            "type": "function_call_output",
            "call_id": "artifact-call",
            "output": line,
            "metadata": {
                "artifact_ref": "prodex-artifact:sc:short-prefix"
            }
        }
    }));

    let plain_summary = lookup_test_path(&plain_shadow, "payload.summary")
        .and_then(Value::as_str)
        .expect("plain summary should exist");
    let artifact_summary = lookup_test_path(&artifact_shadow, "payload.summary")
        .and_then(Value::as_str)
        .expect("artifact summary should exist");

    assert!(plain_summary.contains(tail));
    assert!(!artifact_summary.contains(tail));
    assert!(artifact_summary.contains("ref=prodex-artifact:sc:short-prefix"));
    assert!(artifact_summary.contains("omit=output"));
    assert!(!artifact_summary.contains("; ref="));
}

#[test]
fn super_slim_shadow_tool_output_stores_summary_and_ref() {
    let output = "\nfirst useful output line\n".to_string() + &"artifact data ".repeat(500);
    let shadow = runtime_mem_super_slim_shadow_codex_event(&serde_json::json!({
        "payload": {
            "type": "function_call_output",
            "call_id": "call-1",
            "output": output,
            "artifact": {
                "ref": "prodex://artifact/tool-456"
            }
        }
    }));
    let summary = lookup_test_path(&shadow, "payload.summary")
        .and_then(Value::as_str)
        .expect("tool output summary should exist");

    assert!(summary.starts_with("tool: first useful output line"));
    assert!(summary.contains("b="));
    assert!(summary.contains("t~="));
    assert!(summary.contains("omit=output"));
    assert!(summary.contains("ref=prodex://artifact/tool-456"));
    assert_eq!(
        lookup_test_path(&shadow, "payload.metadata.artifact_ref").and_then(Value::as_str),
        Some("prodex://artifact/tool-456")
    );
    assert_eq!(
        lookup_test_path(&shadow, "payload.output").and_then(Value::as_str),
        Some("ss:omit")
    );
    assert_eq!(
        resolve_schema_tool_response(&runtime_mem_super_slim_codex_schema(), &shadow).as_deref(),
        Some(summary)
    );
}

#[test]
fn super_slim_shadow_falls_back_to_local_summary_when_no_summary_or_ref_exists() {
    let shadow = runtime_mem_super_slim_shadow_codex_event(&serde_json::json!({
        "payload": {
            "type": "custom_tool_call_output",
            "call_id": "call-2",
            "output": "\n\nplain output only\nsecond line has more detail"
        }
    }));
    let summary = lookup_test_path(&shadow, "payload.summary")
        .and_then(Value::as_str)
        .expect("fallback summary should exist");

    assert!(summary.starts_with("tool: plain output only"));
    assert!(summary.contains("b="));
    assert!(summary.contains("t~="));
    assert!(summary.contains("omit=output"));
    assert!(!summary.contains("ref="));
    assert_eq!(
        resolve_schema_tool_response(&runtime_mem_super_slim_codex_schema(), &shadow).as_deref(),
        Some(summary)
    );
}
