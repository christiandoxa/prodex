use super::*;

#[test]
fn super_slim_shadow_events_replaces_later_exact_duplicate_without_semantic_summary() {
    let message = "Important but repeated answer\n".to_string() + &"detail ".repeat(300);
    let events = [
        serde_json::json!({
            "payload": {
                "type": "agent_message",
                "message": message
            }
        }),
        serde_json::json!({
            "payload": {
                "type": "agent_message",
                "message": message
            }
        }),
    ];

    let single_shadow = runtime_mem_super_slim_shadow_codex_event(&events[0]);
    let shadows = runtime_mem_super_slim_shadow_codex_events(events.iter());
    let first_summary = lookup_test_path(&shadows[0], "payload.summary")
        .and_then(Value::as_str)
        .expect("first summary should exist");
    let duplicate_summary = lookup_test_path(&shadows[1], "payload.summary")
        .and_then(Value::as_str)
        .expect("duplicate summary should exist");

    assert_eq!(shadows[0], single_shadow);
    assert!(first_summary.starts_with("a: Important but repeated answer"));
    assert!(duplicate_summary.starts_with("mem dup: original=event[0]"));
    assert!(duplicate_summary.contains("h=sc:"));
    assert!(!duplicate_summary.contains("Important but repeated answer"));
    assert_eq!(
        lookup_test_path(&shadows[1], "payload.message").and_then(Value::as_str),
        Some("ss:omit")
    );
}

#[test]
fn super_slim_shadow_events_use_artifact_ref_for_later_exact_duplicate() {
    let output = "large artifact-backed output\n".to_string() + &"payload ".repeat(300);
    let events = [
        serde_json::json!({
            "payload": {
                "type": "function_call_output",
                "call_id": "call-original",
                "output": output,
                "metadata": {
                    "artifact_ref": "prodex-artifact:sc:tool-output"
                }
            }
        }),
        serde_json::json!({
            "payload": {
                "type": "function_call_output",
                "call_id": "call-dup",
                "output": output
            }
        }),
    ];

    let shadows = runtime_mem_super_slim_shadow_codex_events(events.iter());
    let duplicate_summary = lookup_test_path(&shadows[1], "payload.summary")
        .and_then(Value::as_str)
        .expect("artifact duplicate summary should exist");

    assert!(duplicate_summary.starts_with("prodex-artifact:sc:tool-output"));
    assert!(duplicate_summary.contains("[mem art;"));
    assert!(duplicate_summary.contains("h=sc:"));
    assert!(!duplicate_summary.contains("large artifact-backed output"));
    assert_eq!(
        lookup_test_path(&shadows[1], "payload.metadata.artifact_ref").and_then(Value::as_str),
        Some("prodex-artifact:sc:tool-output")
    );
    assert_eq!(
        lookup_test_path(&shadows[1], "payload.metadata.summary").and_then(Value::as_str),
        Some(duplicate_summary)
    );
}

#[test]
fn super_slim_shadow_events_replaces_later_exact_duplicate_assistant_summary() {
    let summary = "Repeated assistant summary from upstream; same exact text.";
    let events = [
        serde_json::json!({
            "payload": {
                "type": "agent_message",
                "id": "assistant-summary-1",
                "message": "first full assistant message",
                "summary": summary
            }
        }),
        serde_json::json!({
            "payload": {
                "type": "agent_message",
                "id": "assistant-summary-2",
                "message": "different full assistant message",
                "summary": summary
            }
        }),
    ];

    let shadows = runtime_mem_super_slim_shadow_codex_events(events.iter());
    let first_summary = lookup_test_path(&shadows[0], "payload.summary")
        .and_then(Value::as_str)
        .expect("first assistant summary should exist");
    let duplicate_summary = lookup_test_path(&shadows[1], "payload.summary")
        .and_then(Value::as_str)
        .expect("duplicate assistant summary should exist");

    assert_eq!(first_summary, summary);
    assert!(duplicate_summary.starts_with("mem dup: original=assistant-summary-1"));
    assert!(duplicate_summary.contains("h=sc:"));
    assert!(duplicate_summary.contains("b="));
    assert!(!duplicate_summary.contains(summary));
    assert_eq!(
        lookup_test_path(&shadows[1], "payload.message").and_then(Value::as_str),
        Some("ss:omit")
    );
}
