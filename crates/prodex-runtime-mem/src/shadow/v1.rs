use super::summary::{runtime_mem_shadow_summary_for_path, runtime_mem_shadow_summary_for_paths};
use super::*;

pub fn runtime_mem_event_has_super_slim_prompt_reference(event: &Value) -> bool {
    if runtime_mem_event_has_super_slim_v2_prompt_reference(event) {
        return true;
    }
    RUNTIME_MEM_SUPER_SLIM_PROMPT_SUMMARY_PATHS
        .iter()
        .any(|path| {
            runtime_mem_lookup_json_path(event, path).is_some_and(runtime_mem_value_is_text)
        })
        || RUNTIME_MEM_SUPER_SLIM_ARTIFACT_REF_PATHS
            .iter()
            .any(|path| {
                runtime_mem_lookup_json_path(event, path).is_some_and(runtime_mem_value_is_text)
            })
        || runtime_mem_value_contains_artifact_marker(event)
}

fn runtime_mem_event_has_super_slim_v2_prompt_reference(event: &Value) -> bool {
    runtime_mem_lookup_json_path(event, "t").and_then(Value::as_str)
        == Some(RUNTIME_MEM_SUPER_SLIM_V2_USER_EVENT_TYPE)
        && ["s", "r", RUNTIME_MEM_SUPER_SLIM_V2_PREVIOUS_REF_FIELD]
            .iter()
            .any(|path| {
                runtime_mem_lookup_json_path(event, path)
                    .is_some_and(runtime_mem_value_is_text_or_v2_intern_marker)
            })
}

pub fn runtime_mem_super_slim_shadow_codex_event(event: &Value) -> Value {
    let mut shadow = event.clone();
    match runtime_mem_codex_payload_kind(&shadow) {
        RuntimeMemCodexPayloadKind::UserMessage => runtime_mem_shadow_user_message(&mut shadow),
        RuntimeMemCodexPayloadKind::AssistantMessage => {
            runtime_mem_shadow_assistant_message(&mut shadow)
        }
        RuntimeMemCodexPayloadKind::ToolOutput => runtime_mem_shadow_tool_output(&mut shadow),
        _ => {}
    }
    shadow
}

fn runtime_mem_shadow_user_message(event: &mut Value) {
    let summary =
        runtime_mem_first_text_at_paths(event, RUNTIME_MEM_SUPER_SLIM_PROMPT_SUMMARY_PATHS)
            .or_else(|| {
                runtime_mem_shadow_summary_for_paths(
                    event,
                    RUNTIME_MEM_MESSAGE_TEXT_PATHS,
                    "user prompt",
                    "prompt",
                )
            });
    let artifact_ref = runtime_mem_first_artifact_ref_text_at_paths(
        event,
        RUNTIME_MEM_SUPER_SLIM_ARTIFACT_REF_PATHS,
    )
    .or_else(|| runtime_mem_extract_artifact_marker(event));

    if let Some(summary) = summary {
        runtime_mem_set_json_path(
            event,
            "payload.prompt_summary",
            Value::String(summary.clone()),
        );
        runtime_mem_set_json_path(
            event,
            "payload.metadata.prompt_summary",
            Value::String(summary),
        );
    }
    if let Some(artifact_ref) = artifact_ref {
        runtime_mem_set_json_path(
            event,
            "payload.metadata.artifact_ref",
            Value::String(artifact_ref),
        );
    }
    for path in RUNTIME_MEM_MESSAGE_TEXT_PATHS {
        if runtime_mem_lookup_json_path(event, path).is_some() {
            runtime_mem_set_json_path(
                event,
                path,
                Value::String(RUNTIME_MEM_SUPER_SLIM_OMITTED.to_string()),
            );
        }
    }
}

fn runtime_mem_shadow_assistant_message(event: &mut Value) {
    if runtime_mem_lookup_json_path(event, "payload.summary").is_none()
        && let Some(summary) = runtime_mem_shadow_summary_for_paths(
            event,
            RUNTIME_MEM_MESSAGE_TEXT_PATHS,
            "assistant response",
            "message",
        )
    {
        runtime_mem_set_json_path(event, "payload.summary", Value::String(summary));
    }
    for path in RUNTIME_MEM_MESSAGE_TEXT_PATHS {
        if runtime_mem_lookup_json_path(event, path).is_some() {
            runtime_mem_set_json_path(
                event,
                path,
                Value::String(RUNTIME_MEM_SUPER_SLIM_OMITTED.to_string()),
            );
        }
    }
}

fn runtime_mem_shadow_tool_output(event: &mut Value) {
    let summary = runtime_mem_first_text_at_paths(event, RUNTIME_MEM_SUPER_SLIM_TOOL_SUMMARY_PATHS)
        .or_else(|| {
            runtime_mem_shadow_summary_for_path(event, "payload.output", "tool output", "output")
        });
    let artifact_ref =
        runtime_mem_first_artifact_ref_text_at_paths(event, RUNTIME_MEM_SUPER_SLIM_TOOL_REF_PATHS)
            .or_else(|| runtime_mem_extract_artifact_marker(event));

    if let Some(summary) = summary {
        runtime_mem_set_json_path(event, "payload.summary", Value::String(summary.clone()));
        runtime_mem_set_json_path(event, "payload.metadata.summary", Value::String(summary));
    }
    if let Some(artifact_ref) = artifact_ref {
        runtime_mem_set_json_path(
            event,
            "payload.metadata.artifact_ref",
            Value::String(artifact_ref),
        );
    }
    if runtime_mem_lookup_json_path(event, "payload.output").is_some() {
        runtime_mem_set_json_path(
            event,
            "payload.output",
            Value::String(RUNTIME_MEM_SUPER_SLIM_OMITTED.to_string()),
        );
    }
}
