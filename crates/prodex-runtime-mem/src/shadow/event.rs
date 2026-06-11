use super::*;

#[derive(Debug, Clone, Copy)]
pub(crate) struct RuntimeMemEventContentSpec {
    pub(crate) content_paths: &'static [&'static str],
    pub(crate) summary_paths: &'static [&'static str],
    pub(crate) artifact_ref_paths: &'static [&'static str],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RuntimeMemCodexPayloadKind {
    UserMessage,
    AssistantMessage,
    ToolUse,
    ToolOutput,
    Other,
}

pub(crate) fn runtime_mem_codex_payload_kind(event: &Value) -> RuntimeMemCodexPayloadKind {
    let Some(payload) = event.get("payload") else {
        return RuntimeMemCodexPayloadKind::Other;
    };
    let Some(payload_type) = payload.get("type").and_then(Value::as_str) else {
        return RuntimeMemCodexPayloadKind::Other;
    };
    match payload_type {
        "user_message" => RuntimeMemCodexPayloadKind::UserMessage,
        "agent_message" => RuntimeMemCodexPayloadKind::AssistantMessage,
        "message" => match payload.get("role").and_then(Value::as_str) {
            Some("user") => RuntimeMemCodexPayloadKind::UserMessage,
            Some("assistant") => RuntimeMemCodexPayloadKind::AssistantMessage,
            _ => RuntimeMemCodexPayloadKind::Other,
        },
        "function_call" | "custom_tool_call" | "web_search_call" | "exec_command"
        | "local_shell_call" => RuntimeMemCodexPayloadKind::ToolUse,
        "function_call_output" | "custom_tool_call_output" | "exec_command_output" => {
            RuntimeMemCodexPayloadKind::ToolOutput
        }
        _ => RuntimeMemCodexPayloadKind::Other,
    }
}

pub(crate) fn runtime_mem_event_content_spec(event: &Value) -> Option<RuntimeMemEventContentSpec> {
    match runtime_mem_codex_payload_kind(event) {
        RuntimeMemCodexPayloadKind::UserMessage => Some(RuntimeMemEventContentSpec {
            content_paths: RUNTIME_MEM_MESSAGE_TEXT_PATHS,
            summary_paths: RUNTIME_MEM_SUPER_SLIM_PROMPT_SUMMARY_PATHS,
            artifact_ref_paths: RUNTIME_MEM_SUPER_SLIM_ARTIFACT_REF_PATHS,
        }),
        RuntimeMemCodexPayloadKind::AssistantMessage => Some(RuntimeMemEventContentSpec {
            content_paths: RUNTIME_MEM_MESSAGE_TEXT_PATHS,
            summary_paths: RUNTIME_MEM_SUPER_SLIM_ASSISTANT_SUMMARY_PATHS,
            artifact_ref_paths: RUNTIME_MEM_SUPER_SLIM_TOOL_REF_PATHS,
        }),
        RuntimeMemCodexPayloadKind::ToolOutput => Some(RuntimeMemEventContentSpec {
            content_paths: &["payload.output"],
            summary_paths: RUNTIME_MEM_SUPER_SLIM_TOOL_SUMMARY_PATHS,
            artifact_ref_paths: RUNTIME_MEM_SUPER_SLIM_TOOL_REF_PATHS,
        }),
        _ => None,
    }
}
