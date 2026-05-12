use super::*;

#[test]
fn super_slim_v2_omits_tool_name_and_input_when_both_match_schema_defaults() {
    let shadow = runtime_mem_super_slim_v2_shadow_codex_event(&serde_json::json!({
        "payload": {
            "type": "custom_tool_call",
            "call_id": "call-default",
            "name": "tool",
            "action": "tool call"
        }
    }));

    assert_eq!(shadow["t"].as_str(), Some("pm2:tu"));
    assert_eq!(shadow.get("n"), None);
    assert_eq!(shadow.get("c"), None);

    let fields = v2_schema_fields("prodex-v2-tool-use");
    assert_eq!(
        resolve_v2_schema_string(&fields["toolName"], &shadow).as_deref(),
        Some("tool")
    );
    assert_eq!(
        resolve_v2_schema_string(&fields["toolInput"], &shadow).as_deref(),
        Some("tool call")
    );
}

#[test]
fn super_slim_v2_omits_tool_input_when_it_duplicates_tool_name() {
    let shadow = runtime_mem_super_slim_v2_shadow_codex_event(&serde_json::json!({
        "payload": {
            "type": "function_call",
            "call_id": "call-dup",
            "name": "web_search"
        }
    }));

    assert_eq!(shadow["t"].as_str(), Some("pm2:tu"));
    assert_eq!(shadow["n"].as_str(), Some("web_search"));
    assert_eq!(shadow.get("c"), None);

    let fields = v2_schema_fields("prodex-v2-tool-use");
    assert_eq!(
        resolve_v2_schema_string(&fields["toolName"], &shadow).as_deref(),
        Some("web_search")
    );
    assert_eq!(
        resolve_v2_schema_string(&fields["toolInput"], &shadow).as_deref(),
        Some("web_search")
    );
}

#[test]
fn super_slim_v2_omits_default_tool_name_when_input_preserves_reader_output() {
    let shadow = runtime_mem_super_slim_v2_shadow_codex_event(&serde_json::json!({
        "payload": {
            "type": "custom_tool_call",
            "call_id": "call-tool",
            "name": "tool",
            "action": "run diagnostics"
        }
    }));

    assert_eq!(shadow["t"].as_str(), Some("pm2:tu"));
    assert_eq!(shadow.get("n"), None);
    assert_eq!(shadow["c"].as_str(), Some("run diagnostics"));

    let fields = v2_schema_fields("prodex-v2-tool-use");
    assert_eq!(
        resolve_v2_schema_string(&fields["toolName"], &shadow).as_deref(),
        Some("tool")
    );
    assert_eq!(
        resolve_v2_schema_string(&fields["toolInput"], &shadow).as_deref(),
        Some("run diagnostics")
    );
}

#[test]
fn super_slim_v2_shadow_events_mark_consecutive_duplicate_artifact_refs() {
    let artifact_ref = "psc:repeat-ref";
    let events = [
        serde_json::json!({
            "payload": {
                "type": "user_message",
                "message": "first artifact-backed prompt",
                "metadata": {
                    "artifact_ref": artifact_ref
                }
            }
        }),
        serde_json::json!({
            "payload": {
                "type": "exec_command_output",
                "call_id": "call-repeat",
                "output": "same artifact-backed output",
                "metadata": {
                    "artifact_ref": artifact_ref
                }
            }
        }),
        serde_json::json!({
            "payload": {
                "type": "user_message",
                "message": "third consecutive artifact-backed prompt",
                "metadata": {
                    "artifact_ref": artifact_ref
                }
            }
        }),
    ];

    let shadows = runtime_mem_super_slim_v2_shadow_codex_events(events.iter());

    assert_eq!(shadows[0]["r"].as_str(), Some(artifact_ref));
    assert_eq!(
        shadows[0].get(RUNTIME_MEM_SUPER_SLIM_V2_PREVIOUS_REF_FIELD),
        None
    );
    assert_eq!(shadows[1].get("r"), None);
    assert_eq!(
        shadows[1][RUNTIME_MEM_SUPER_SLIM_V2_PREVIOUS_REF_FIELD].as_str(),
        Some(RUNTIME_MEM_SUPER_SLIM_V2_PREVIOUS_REF_MARKER)
    );
    assert_eq!(shadows[2].get("r"), None);
    assert_eq!(
        shadows[2][RUNTIME_MEM_SUPER_SLIM_V2_PREVIOUS_REF_FIELD].as_str(),
        Some(RUNTIME_MEM_SUPER_SLIM_V2_PREVIOUS_REF_MARKER)
    );

    let fields = v2_schema_fields("prodex-v2-tool-result");
    assert_eq!(
        resolve_v2_schema_string(&fields["toolResponse"], &shadows[1]).as_deref(),
        Some(RUNTIME_MEM_SUPER_SLIM_V2_PREVIOUS_REF_MARKER)
    );

    let legacy_tool_response = serde_json::json!({
        "coalesce": ["s", "r", { "value": RUNTIME_MEM_SUPER_SLIM_TOOL_OMITTED }]
    });
    assert_eq!(
        resolve_v2_schema_string(&legacy_tool_response, &shadows[1]).as_deref(),
        Some(RUNTIME_MEM_SUPER_SLIM_TOOL_OMITTED)
    );
}

#[test]
fn super_slim_v2_shadow_events_do_not_mark_non_consecutive_refs() {
    let artifact_ref = "psc:not-consecutive";
    let events = [
        serde_json::json!({
            "payload": {
                "type": "user_message",
                "message": "first artifact-backed prompt",
                "metadata": {
                    "artifact_ref": artifact_ref
                }
            }
        }),
        serde_json::json!({
            "payload": {
                "type": "agent_message",
                "message": "assistant event breaks adjacency",
                "summary": "assistant summary"
            }
        }),
        serde_json::json!({
            "payload": {
                "type": "exec_command_output",
                "call_id": "call-later",
                "output": "same ref after non-ref event",
                "metadata": {
                    "artifact_ref": artifact_ref
                }
            }
        }),
    ];

    let shadows = runtime_mem_super_slim_v2_shadow_codex_events(events.iter());

    assert_eq!(shadows[0]["r"].as_str(), Some(artifact_ref));
    assert_eq!(shadows[2]["r"].as_str(), Some(artifact_ref));
    assert_eq!(
        shadows[2].get(RUNTIME_MEM_SUPER_SLIM_V2_PREVIOUS_REF_FIELD),
        None
    );
}

#[test]
fn super_slim_v2_schema_reads_previous_ref_marker_and_legacy_full_refs() {
    let marker_event = serde_json::json!({
        "t": RUNTIME_MEM_SUPER_SLIM_V2_USER_EVENT_TYPE,
        RUNTIME_MEM_SUPER_SLIM_V2_PREVIOUS_REF_FIELD: RUNTIME_MEM_SUPER_SLIM_V2_PREVIOUS_REF_MARKER
    });
    let legacy_marker_event = serde_json::json!({
        "t": RUNTIME_MEM_SUPER_SLIM_V2_USER_EVENT_TYPE,
        RUNTIME_MEM_SUPER_SLIM_V2_PREVIOUS_REF_FIELD: RUNTIME_MEM_SUPER_SLIM_V2_PREVIOUS_REF_MARKER_LEGACY
    });
    let legacy_event = serde_json::json!({
        "t": RUNTIME_MEM_SUPER_SLIM_V2_USER_EVENT_TYPE,
        "r": "psc:legacy-full-ref"
    });
    let fields = v2_schema_fields("prodex-v2-user-message");

    assert_eq!(
        resolve_v2_schema_string(&fields["prompt"], &marker_event).as_deref(),
        Some(RUNTIME_MEM_SUPER_SLIM_V2_PREVIOUS_REF_MARKER)
    );
    assert_eq!(
        resolve_v2_schema_string(&fields["prompt"], &legacy_marker_event).as_deref(),
        Some(RUNTIME_MEM_SUPER_SLIM_V2_PREVIOUS_REF_MARKER_LEGACY)
    );
    assert_eq!(
        resolve_v2_schema_string(&fields["prompt"], &legacy_event).as_deref(),
        Some("psc:legacy-full-ref")
    );
    assert!(runtime_mem_event_has_super_slim_prompt_reference(
        &marker_event
    ));
    assert!(runtime_mem_event_has_super_slim_prompt_reference(
        &legacy_marker_event
    ));
}

#[test]
fn super_slim_v2_schema_still_reads_legacy_tool_name_and_input_fields() {
    let legacy = serde_json::json!({
        "t": "pm2:tu",
        "i": "call-legacy",
        "n": "exec_command",
        "c": "cargo test -q"
    });
    let fields = v2_schema_fields("prodex-v2-tool-use");

    assert_eq!(
        resolve_v2_schema_string(&fields["toolId"], &legacy).as_deref(),
        Some("call-legacy")
    );
    assert_eq!(
        resolve_v2_schema_string(&fields["toolName"], &legacy).as_deref(),
        Some("exec_command")
    );
    assert_eq!(
        resolve_v2_schema_string(&fields["toolInput"], &legacy).as_deref(),
        Some("cargo test -q")
    );
}
