use super::*;
use std::collections::HashMap;

#[derive(Debug, Default)]
pub(crate) struct RuntimeMemConversationElisionState {
    total_events: usize,
    tool_commands: HashMap<String, String>,
}

impl RuntimeMemConversationElisionState {
    pub(crate) fn new(total_events: usize) -> Self {
        Self {
            total_events,
            tool_commands: HashMap::new(),
        }
    }

    pub(crate) fn remember_event(&mut self, event: &Value) {
        if runtime_mem_codex_payload_kind(event) != RuntimeMemCodexPayloadKind::ToolUse {
            return;
        }
        let Some(tool_id) = runtime_mem_first_text_at_paths(event, &["payload.call_id"]) else {
            return;
        };
        let Some(command) = runtime_mem_first_text_at_paths(
            event,
            &[
                "payload.command",
                "payload.action.command",
                "payload.action",
                "payload.name",
            ],
        ) else {
            return;
        };
        self.tool_commands.entry(tool_id).or_insert(command);
    }

    fn command_for_event(&self, event: &Value) -> Option<&str> {
        let tool_id = runtime_mem_first_text_at_paths(event, &["payload.call_id"])?;
        self.tool_commands.get(&tool_id).map(String::as_str)
    }

    fn event_is_old(&self, index: usize) -> bool {
        index.saturating_add(RUNTIME_MEM_CONVERSATION_ELISION_RECENT_EVENT_WINDOW)
            < self.total_events
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RuntimeMemConversationElisionKind {
    User,
    Assistant,
    Tool,
}

impl RuntimeMemConversationElisionKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Assistant => "assistant",
            Self::Tool => "tool",
        }
    }
}

pub(crate) fn runtime_mem_elide_old_conversation_event(
    event: &Value,
    shadow: &mut Value,
    index: usize,
    elision_state: &RuntimeMemConversationElisionState,
) {
    if !elision_state.event_is_old(index) {
        return;
    }
    let Some((kind, spec)) = runtime_mem_conversation_elision_spec(event) else {
        return;
    };

    let Some(content) = runtime_mem_first_text_path_at_paths(event, spec.content_paths)
        .map(|(_, content)| content.trim().to_string())
        .filter(|content| content.len() >= RUNTIME_MEM_CONVERSATION_ELISION_MIN_CONTENT_BYTES)
    else {
        return;
    };
    let artifact_ref =
        runtime_mem_first_prodex_artifact_ref_at_paths(event, spec.artifact_ref_paths, &content);
    let command = (kind == RuntimeMemConversationElisionKind::Tool)
        .then(|| elision_state.command_for_event(event))
        .flatten();
    let summary =
        runtime_mem_conversation_elision_summary(kind, &content, command, artifact_ref.as_deref());
    for path in spec.summary_paths {
        runtime_mem_set_json_path(shadow, path, Value::String(summary.clone()));
    }
    if let Some(artifact_ref) = artifact_ref {
        runtime_mem_set_json_path(
            shadow,
            "payload.metadata.artifact_ref",
            Value::String(artifact_ref),
        );
    }
}

fn runtime_mem_conversation_elision_spec(
    event: &Value,
) -> Option<(
    RuntimeMemConversationElisionKind,
    RuntimeMemEventContentSpec,
)> {
    let kind = match runtime_mem_codex_payload_kind(event) {
        RuntimeMemCodexPayloadKind::UserMessage => RuntimeMemConversationElisionKind::User,
        RuntimeMemCodexPayloadKind::AssistantMessage => {
            RuntimeMemConversationElisionKind::Assistant
        }
        RuntimeMemCodexPayloadKind::ToolOutput => RuntimeMemConversationElisionKind::Tool,
        _ => return None,
    };
    Some((kind, runtime_mem_event_content_spec(event)?))
}
