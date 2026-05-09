use crate::dictionary::{
    RuntimeMemSuperSlimV2ArtifactRefDedupeState, RuntimeMemSuperSlimV2InternState,
    runtime_mem_super_slim_v2_compact_dictionary_events,
    runtime_mem_value_is_text_or_v2_intern_marker,
};
use crate::{
    CLAUDE_MEM_FULL_FLAG, CLAUDE_MEM_FULL_PREFIX, CLAUDE_MEM_PREFIX, CLAUDE_MEM_SUPER_SLIM_FLAG,
    CLAUDE_MEM_SUPER_SLIM_PREFIX, RUNTIME_MEM_CONVERSATION_ELISION_FACT_CHAR_LIMIT,
    RUNTIME_MEM_CONVERSATION_ELISION_MAX_FACTS, RUNTIME_MEM_CONVERSATION_ELISION_MIN_CONTENT_BYTES,
    RUNTIME_MEM_CONVERSATION_ELISION_RECENT_EVENT_WINDOW,
    RUNTIME_MEM_CONVERSATION_ELISION_SCAN_CHAR_LIMIT,
    RUNTIME_MEM_CONVERSATION_LEDGER_OBJECTIVE_CHAR_LIMIT,
    RUNTIME_MEM_CONVERSATION_LEDGER_SUMMARY_CHAR_LIMIT, RUNTIME_MEM_MESSAGE_TEXT_PATHS,
    RUNTIME_MEM_SUPER_SLIM_ARTIFACT_REF_PATHS, RUNTIME_MEM_SUPER_SLIM_ASSISTANT_SUMMARY_PATHS,
    RUNTIME_MEM_SUPER_SLIM_OMITTED, RUNTIME_MEM_SUPER_SLIM_PROMPT_SUMMARY_PATHS,
    RUNTIME_MEM_SUPER_SLIM_REFERENCED_SUMMARY_PREFIX_CHAR_LIMIT,
    RUNTIME_MEM_SUPER_SLIM_SUMMARY_PREFIX_CHAR_LIMIT, RUNTIME_MEM_SUPER_SLIM_TOOL_REF_PATHS,
    RUNTIME_MEM_SUPER_SLIM_TOOL_SUMMARY_PATHS, RUNTIME_MEM_SUPER_SLIM_V2_ASSISTANT_EVENT_TYPE,
    RUNTIME_MEM_SUPER_SLIM_V2_DEFAULT_TOOL_INPUT, RUNTIME_MEM_SUPER_SLIM_V2_DEFAULT_TOOL_NAME,
    RUNTIME_MEM_SUPER_SLIM_V2_PREVIOUS_REF_FIELD, RUNTIME_MEM_SUPER_SLIM_V2_TOOL_RESULT_EVENT_TYPE,
    RUNTIME_MEM_SUPER_SLIM_V2_TOOL_USE_EVENT_TYPE, RUNTIME_MEM_SUPER_SLIM_V2_USER_EVENT_TYPE,
    RuntimeMemTranscriptMode, runtime_mem_approx_token_count,
    runtime_mem_artifact_aliases_from_text, runtime_mem_artifact_recall_summary,
    runtime_mem_artifact_ref_tokens, runtime_mem_content_hash,
    runtime_mem_duplicate_recall_summary, runtime_mem_extract_artifact_marker,
    runtime_mem_first_artifact_ref_text_at_paths, runtime_mem_first_prodex_artifact_ref_at_paths,
    runtime_mem_first_text_at_paths, runtime_mem_first_text_path_at_paths,
    runtime_mem_first_useful_line, runtime_mem_lookup_json_path,
    runtime_mem_normalize_prodex_artifact_ref, runtime_mem_parse_artifact_ref_token,
    runtime_mem_prodex_artifact_ref, runtime_mem_prompt_term_is_path, runtime_mem_set_json_path,
    runtime_mem_truncate_chars,
};
use serde_json::Value;
use std::collections::HashMap;
use std::ffi::OsString;

mod conversation;
mod summary;

use self::conversation::runtime_mem_conversation_elision_summary;
use self::summary::{
    runtime_mem_shadow_summary_for_path, runtime_mem_shadow_summary_for_paths,
    runtime_mem_summary_has_critical_signal, runtime_mem_value_contains_text,
};

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

pub fn runtime_mem_super_slim_shadow_codex_events<'a>(
    events: impl IntoIterator<Item = &'a Value>,
) -> Vec<Value> {
    let mut dedupe_state = RuntimeMemEventDedupeState::default();
    let events = events.into_iter().collect::<Vec<_>>();
    let mut elision_state = RuntimeMemConversationElisionState::new(events.len());
    events
        .iter()
        .enumerate()
        .map(|(index, event)| {
            elision_state.remember_event(event);
            let mut shadow = runtime_mem_super_slim_shadow_codex_event_with_dedupe(
                event,
                index,
                &mut dedupe_state,
            );
            runtime_mem_elide_old_conversation_event(event, &mut shadow, index, &elision_state);
            shadow
        })
        .collect()
}

pub fn runtime_mem_super_slim_v2_shadow_codex_event(event: &Value) -> Value {
    runtime_mem_super_slim_v2_shadow_from_v1_shadow(&runtime_mem_super_slim_shadow_codex_event(
        event,
    ))
}

pub fn runtime_mem_super_slim_v2_shadow_codex_events<'a>(
    events: impl IntoIterator<Item = &'a Value>,
) -> Vec<Value> {
    let mut ref_dedupe_state = RuntimeMemSuperSlimV2ArtifactRefDedupeState::default();
    let events = runtime_mem_super_slim_shadow_codex_events(events)
        .iter()
        .map(runtime_mem_super_slim_v2_shadow_from_v1_shadow)
        .map(|event| ref_dedupe_state.dedupe_consecutive_event_ref(event))
        .collect();
    runtime_mem_super_slim_v2_compact_dictionary_events(events)
}

pub fn runtime_mem_super_slim_v2_expand_interned_events(
    events: impl IntoIterator<Item = Value>,
) -> Vec<Value> {
    let mut intern_state = RuntimeMemSuperSlimV2InternState::default();
    events
        .into_iter()
        .filter_map(|event| intern_state.expand_event(event))
        .collect()
}

pub fn runtime_mem_extract_mode_with_detail(
    args: &[OsString],
) -> (Option<RuntimeMemTranscriptMode>, Vec<OsString>) {
    let Some(first) = args.first().and_then(|arg| arg.to_str()) else {
        return (None, args.to_vec());
    };
    if first == CLAUDE_MEM_SUPER_SLIM_PREFIX {
        return (
            Some(RuntimeMemTranscriptMode::SuperSlim),
            args[1..].to_vec(),
        );
    }
    if first == CLAUDE_MEM_FULL_PREFIX {
        return (Some(RuntimeMemTranscriptMode::Full), args[1..].to_vec());
    }
    if first != CLAUDE_MEM_PREFIX {
        return (None, args.to_vec());
    }
    if args
        .get(1)
        .and_then(|arg| arg.to_str())
        .is_some_and(|arg| arg == CLAUDE_MEM_SUPER_SLIM_FLAG)
    {
        return (
            Some(RuntimeMemTranscriptMode::SuperSlim),
            args[2..].to_vec(),
        );
    }
    if args
        .get(1)
        .and_then(|arg| arg.to_str())
        .is_some_and(|arg| arg == CLAUDE_MEM_FULL_FLAG)
    {
        return (Some(RuntimeMemTranscriptMode::Full), args[2..].to_vec());
    }
    (Some(RuntimeMemTranscriptMode::Slim), args[1..].to_vec())
}

#[derive(Debug, Clone, Copy)]
struct RuntimeMemEventContentSpec {
    content_paths: &'static [&'static str],
    summary_paths: &'static [&'static str],
    artifact_ref_paths: &'static [&'static str],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RuntimeMemCodexPayloadKind {
    UserMessage,
    AssistantMessage,
    ToolUse,
    ToolOutput,
    Other,
}

#[derive(Debug, Clone)]
struct RuntimeMemSeenEventContent {
    original_id: String,
    artifact_ref: Option<String>,
}

#[derive(Debug, Default)]
struct RuntimeMemEventDedupeState {
    seen_content: HashMap<String, RuntimeMemSeenEventContent>,
    seen_assistant_summary: HashMap<String, RuntimeMemSeenEventContent>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RuntimeMemDedupeReplacement {
    summary: String,
    artifact_ref: Option<String>,
}

#[derive(Debug, Default)]
struct RuntimeMemConversationElisionState {
    total_events: usize,
    tool_commands: HashMap<String, String>,
}

impl RuntimeMemConversationElisionState {
    fn new(total_events: usize) -> Self {
        Self {
            total_events,
            tool_commands: HashMap::new(),
        }
    }

    fn remember_event(&mut self, event: &Value) {
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
enum RuntimeMemConversationElisionKind {
    User,
    Assistant,
    Tool,
}

impl RuntimeMemConversationElisionKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Assistant => "assistant",
            Self::Tool => "tool",
        }
    }
}

pub(crate) fn runtime_mem_codex_payload_kind(event: &Value) -> RuntimeMemCodexPayloadKind {
    let Some(payload_type) =
        runtime_mem_lookup_json_path(event, "payload.type").and_then(Value::as_str)
    else {
        return RuntimeMemCodexPayloadKind::Other;
    };
    match payload_type {
        "user_message" => RuntimeMemCodexPayloadKind::UserMessage,
        "agent_message" => RuntimeMemCodexPayloadKind::AssistantMessage,
        "message" => {
            match runtime_mem_lookup_json_path(event, "payload.role").and_then(Value::as_str) {
                Some("user") => RuntimeMemCodexPayloadKind::UserMessage,
                Some("assistant") => RuntimeMemCodexPayloadKind::AssistantMessage,
                _ => RuntimeMemCodexPayloadKind::Other,
            }
        }
        "function_call" | "custom_tool_call" | "web_search_call" | "exec_command"
        | "local_shell_call" => RuntimeMemCodexPayloadKind::ToolUse,
        "function_call_output" | "custom_tool_call_output" | "exec_command_output" => {
            RuntimeMemCodexPayloadKind::ToolOutput
        }
        _ => RuntimeMemCodexPayloadKind::Other,
    }
}

impl RuntimeMemEventDedupeState {
    fn replacement_for_optional_content(
        &mut self,
        id: String,
        content: &str,
        artifact_ref: Option<String>,
    ) -> Option<RuntimeMemDedupeReplacement> {
        runtime_mem_replacement_for_optional_seen(&mut self.seen_content, id, content, artifact_ref)
    }

    fn replacement_for_optional_assistant_summary(
        &mut self,
        id: String,
        summary: &str,
    ) -> Option<RuntimeMemDedupeReplacement> {
        runtime_mem_replacement_for_optional_seen(
            &mut self.seen_assistant_summary,
            id,
            summary,
            runtime_mem_prodex_artifact_ref(None, summary),
        )
    }
}

fn runtime_mem_replacement_for_optional_seen(
    seen_by_content: &mut HashMap<String, RuntimeMemSeenEventContent>,
    id: String,
    content: &str,
    artifact_ref: Option<String>,
) -> Option<RuntimeMemDedupeReplacement> {
    if let Some(seen) = seen_by_content.get_mut(content) {
        if seen.artifact_ref.is_none() {
            seen.artifact_ref = artifact_ref.clone();
        }
        let content_hash = runtime_mem_content_hash(content);
        if let Some(artifact_ref) = artifact_ref.or_else(|| seen.artifact_ref.clone()) {
            return Some(RuntimeMemDedupeReplacement {
                summary: runtime_mem_artifact_recall_summary(
                    &artifact_ref,
                    &content_hash,
                    content.len(),
                ),
                artifact_ref: Some(artifact_ref),
            });
        }
        return Some(RuntimeMemDedupeReplacement {
            summary: runtime_mem_duplicate_recall_summary(
                &seen.original_id,
                &content_hash,
                content.len(),
            ),
            artifact_ref: None,
        });
    }

    seen_by_content.insert(
        content.to_string(),
        RuntimeMemSeenEventContent {
            original_id: id,
            artifact_ref,
        },
    );
    None
}

fn runtime_mem_super_slim_shadow_codex_event_with_dedupe(
    event: &Value,
    index: usize,
    dedupe_state: &mut RuntimeMemEventDedupeState,
) -> Value {
    let spec = runtime_mem_event_content_spec(event);
    let replacement = spec.and_then(|spec| {
        let content_replacement = runtime_mem_first_text_path_at_paths(event, spec.content_paths)
            .and_then(|(_, content)| {
                let artifact_ref = runtime_mem_first_prodex_artifact_ref_at_paths(
                    event,
                    spec.artifact_ref_paths,
                    &content,
                );
                dedupe_state.replacement_for_optional_content(
                    runtime_mem_event_dedupe_id(event, index),
                    &content,
                    artifact_ref,
                )
            });
        content_replacement
            .or_else(|| {
                runtime_mem_assistant_summary_duplicate_replacement(event, index, dedupe_state)
            })
            .map(|replacement| (spec, replacement))
    });
    let mut shadow = runtime_mem_super_slim_shadow_codex_event(event);
    if let Some((spec, replacement)) = replacement {
        for path in spec.summary_paths {
            runtime_mem_set_json_path(
                &mut shadow,
                path,
                Value::String(replacement.summary.clone()),
            );
        }
        if let Some(artifact_ref) = replacement.artifact_ref {
            runtime_mem_set_json_path(
                &mut shadow,
                "payload.metadata.artifact_ref",
                Value::String(artifact_ref),
            );
        }
    }
    shadow
}

fn runtime_mem_elide_old_conversation_event(
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

fn runtime_mem_assistant_summary_duplicate_replacement(
    event: &Value,
    index: usize,
    dedupe_state: &mut RuntimeMemEventDedupeState,
) -> Option<RuntimeMemDedupeReplacement> {
    if runtime_mem_codex_payload_kind(event) != RuntimeMemCodexPayloadKind::AssistantMessage {
        return None;
    }
    let summary = runtime_mem_lookup_json_path(event, "payload.summary")?
        .as_str()?
        .trim();
    if summary.is_empty() {
        return None;
    }
    dedupe_state.replacement_for_optional_assistant_summary(
        runtime_mem_event_dedupe_id(event, index),
        summary,
    )
}

fn runtime_mem_event_content_spec(event: &Value) -> Option<RuntimeMemEventContentSpec> {
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

fn runtime_mem_event_dedupe_id(event: &Value, index: usize) -> String {
    runtime_mem_first_text_at_paths(event, &["payload.call_id", "payload.id", "id"])
        .unwrap_or_else(|| format!("event[{index}]"))
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

pub(crate) fn runtime_mem_value_is_text(value: &Value) -> bool {
    value.as_str().is_some_and(|text| !text.trim().is_empty())
}

pub(crate) fn runtime_mem_value_contains_artifact_marker(value: &Value) -> bool {
    if runtime_mem_extract_artifact_marker(value).is_some() {
        return true;
    }
    runtime_mem_value_contains_text(value, "prodex smart context artifact")
}
