use super::summary::runtime_mem_summary_has_critical_signal;
use super::*;

pub(crate) fn runtime_mem_super_slim_v2_shadow_from_v1_shadow(event: &Value) -> Value {
    match runtime_mem_codex_payload_kind(event) {
        RuntimeMemCodexPayloadKind::UserMessage => {
            let mut shadow =
                runtime_mem_short_shadow_event(RUNTIME_MEM_SUPER_SLIM_V2_USER_EVENT_TYPE);
            let summary =
                runtime_mem_first_text_at_paths(event, RUNTIME_MEM_SUPER_SLIM_PROMPT_SUMMARY_PATHS);
            let artifact_ref = runtime_mem_first_artifact_ref_text_at_paths(
                event,
                RUNTIME_MEM_SUPER_SLIM_ARTIFACT_REF_PATHS,
            );
            if let Some(artifact_ref) = artifact_ref.as_ref() {
                shadow.insert("r".to_string(), Value::String(artifact_ref.clone()));
            }
            if runtime_mem_super_slim_v2_should_keep_summary(
                summary.as_deref(),
                artifact_ref.as_deref(),
            ) && let Some(summary) = summary
            {
                shadow.insert("s".to_string(), Value::String(summary));
            }
            Value::Object(shadow)
        }
        RuntimeMemCodexPayloadKind::AssistantMessage => {
            let mut shadow =
                runtime_mem_short_shadow_event(RUNTIME_MEM_SUPER_SLIM_V2_ASSISTANT_EVENT_TYPE);
            if let Some(summary) = runtime_mem_first_text_at_paths(
                event,
                RUNTIME_MEM_SUPER_SLIM_ASSISTANT_SUMMARY_PATHS,
            ) {
                shadow.insert("s".to_string(), Value::String(summary));
            }
            Value::Object(shadow)
        }
        RuntimeMemCodexPayloadKind::ToolUse => {
            let mut shadow =
                runtime_mem_short_shadow_event(RUNTIME_MEM_SUPER_SLIM_V2_TOOL_USE_EVENT_TYPE);
            if let Some(tool_id) = runtime_mem_first_text_at_paths(event, &["payload.call_id"]) {
                shadow.insert("i".to_string(), Value::String(tool_id));
            }
            let tool_name =
                runtime_mem_first_text_at_paths(event, &["payload.name", "payload.type"]);
            let tool_input = runtime_mem_first_text_at_paths(
                event,
                &[
                    "payload.command",
                    "payload.action.command",
                    "payload.action",
                    "payload.name",
                ],
            );
            runtime_mem_insert_super_slim_v2_tool_use_fields(&mut shadow, tool_name, tool_input);
            Value::Object(shadow)
        }
        RuntimeMemCodexPayloadKind::ToolOutput => {
            let mut shadow =
                runtime_mem_short_shadow_event(RUNTIME_MEM_SUPER_SLIM_V2_TOOL_RESULT_EVENT_TYPE);
            if let Some(tool_id) = runtime_mem_first_text_at_paths(event, &["payload.call_id"]) {
                shadow.insert("i".to_string(), Value::String(tool_id));
            }
            let summary =
                runtime_mem_first_text_at_paths(event, RUNTIME_MEM_SUPER_SLIM_TOOL_SUMMARY_PATHS);
            let artifact_ref = runtime_mem_first_artifact_ref_text_at_paths(
                event,
                RUNTIME_MEM_SUPER_SLIM_TOOL_REF_PATHS,
            );
            if let Some(artifact_ref) = artifact_ref.as_ref() {
                shadow.insert("r".to_string(), Value::String(artifact_ref.clone()));
            }
            if runtime_mem_super_slim_v2_should_keep_summary(
                summary.as_deref(),
                artifact_ref.as_deref(),
            ) && let Some(summary) = summary
            {
                shadow.insert("s".to_string(), Value::String(summary));
            }
            Value::Object(shadow)
        }
        RuntimeMemCodexPayloadKind::Other => event.clone(),
    }
}

pub(crate) fn runtime_mem_insert_super_slim_v2_tool_use_fields(
    shadow: &mut serde_json::Map<String, Value>,
    tool_name: Option<String>,
    tool_input: Option<String>,
) {
    match (tool_name, tool_input) {
        (Some(tool_name), Some(tool_input))
            if tool_name == RUNTIME_MEM_SUPER_SLIM_V2_DEFAULT_TOOL_NAME
                && tool_input == RUNTIME_MEM_SUPER_SLIM_V2_DEFAULT_TOOL_INPUT => {}
        (Some(tool_name), Some(tool_input))
            if tool_name == RUNTIME_MEM_SUPER_SLIM_V2_DEFAULT_TOOL_NAME =>
        {
            shadow.insert("c".to_string(), Value::String(tool_input));
        }
        (Some(tool_name), Some(tool_input)) if tool_input == tool_name => {
            shadow.insert("n".to_string(), Value::String(tool_name));
        }
        (Some(tool_name), Some(tool_input)) => {
            shadow.insert("n".to_string(), Value::String(tool_name));
            shadow.insert("c".to_string(), Value::String(tool_input));
        }
        (Some(tool_name), None) => {
            shadow.insert("n".to_string(), Value::String(tool_name));
        }
        (None, Some(tool_input)) if tool_input != RUNTIME_MEM_SUPER_SLIM_V2_DEFAULT_TOOL_INPUT => {
            shadow.insert("c".to_string(), Value::String(tool_input));
        }
        (None, Some(_)) | (None, None) => {}
    }
}

pub(crate) fn runtime_mem_super_slim_v2_should_keep_summary(
    summary: Option<&str>,
    artifact_ref: Option<&str>,
) -> bool {
    let Some(summary) = summary else {
        return false;
    };
    artifact_ref.is_none() || runtime_mem_summary_has_critical_signal(summary)
}

pub(crate) fn runtime_mem_short_shadow_event(event_type: &str) -> serde_json::Map<String, Value> {
    let mut shadow = serde_json::Map::new();
    shadow.insert("t".to_string(), Value::String(event_type.to_string()));
    shadow
}
