use serde_json::Value;
use std::collections::HashMap;
use std::ffi::OsString;

mod artifact_refs;
mod recall_selection;
mod schema;
mod watch;

pub use artifact_refs::runtime_mem_content_hash;
pub(crate) use artifact_refs::{
    runtime_mem_artifact_aliases_from_text, runtime_mem_artifact_recall_summary,
    runtime_mem_artifact_ref_tokens, runtime_mem_duplicate_recall_summary,
    runtime_mem_extract_artifact_marker, runtime_mem_first_artifact_ref_text_at_paths,
    runtime_mem_first_prodex_artifact_ref_at_paths, runtime_mem_normalize_prodex_artifact_ref,
    runtime_mem_parse_artifact_ref_token, runtime_mem_prodex_artifact_ref,
};
pub(crate) use recall_selection::runtime_mem_prompt_term_is_path;
pub use recall_selection::{
    RUNTIME_MEM_DEFAULT_CAPSULE_CONDENSED_TOKEN_BUDGET,
    RUNTIME_MEM_DEFAULT_CAPSULE_LARGE_TOKEN_BUDGET,
    RUNTIME_MEM_DEFAULT_CAPSULE_MINIMAL_TOKEN_BUDGET, RUNTIME_MEM_DEFAULT_RECENT_WINDOW_SECONDS,
    RUNTIME_MEM_SUPER_CAPSULE_CONDENSED_TOKEN_BUDGET, RUNTIME_MEM_SUPER_CAPSULE_LARGE_TOKEN_BUDGET,
    RUNTIME_MEM_SUPER_CAPSULE_MINIMAL_TOKEN_BUDGET, RuntimeMemAutoCapsuleSelectionContext,
    RuntimeMemCapsuleBudget, RuntimeMemCapsuleBudgetMode, RuntimeMemCapsuleBudgetTier,
    RuntimeMemCapsuleMetadata, RuntimeMemCapsulePriority, RuntimeMemCapsuleSelection,
    RuntimeMemCapsuleSelectionContext, RuntimeMemCapsuleSelectionEntry,
    RuntimeMemRecallCapsuleMetadata, RuntimeMemRecallDedupeEntry, RuntimeMemRecallDedupeItem,
    RuntimeMemRecallDedupeReason, RuntimeMemRecallIntent, runtime_mem_capsule_budget_tier,
    runtime_mem_capsule_token_budget, runtime_mem_capsule_token_budget_for_tier,
    runtime_mem_classify_capsule, runtime_mem_dedupe_recall_content, runtime_mem_select_capsules,
    runtime_mem_select_capsules_auto, runtime_mem_select_capsules_for_recall_diet,
    runtime_mem_select_capsules_with_recall_intent,
};
pub use schema::{
    runtime_mem_codex_schema_for_mode, runtime_mem_default_codex_schema,
    runtime_mem_full_codex_schema, runtime_mem_super_slim_codex_schema,
    runtime_mem_super_slim_v1_codex_schema,
};
pub use watch::{
    ensure_runtime_mem_codex_watch_for_home, ensure_runtime_mem_codex_watch_for_home_at_path,
    ensure_runtime_mem_codex_watch_for_home_at_path_with_mode,
    ensure_runtime_mem_codex_watch_for_home_with_mode,
    ensure_runtime_mem_codex_watch_for_sessions_root,
    ensure_runtime_mem_codex_watch_for_sessions_root_with_mode,
    ensure_runtime_mem_prodex_observer_for_home_and_root, runtime_mem_claude_plugin_dir,
    runtime_mem_claude_plugin_dir_from_home, runtime_mem_claude_plugin_manifest_path,
    runtime_mem_data_dir_from_home, runtime_mem_prodex_claude_wrapper_path_from_root,
    runtime_mem_settings_path_from_home, runtime_mem_transcript_watch_config_path_from_home,
};

const DEFAULT_CLAUDE_CONFIG_DIR_NAME: &str = ".claude";
const CLAUDE_MEM_DATA_DIR_NAME: &str = ".claude-mem";
const CLAUDE_MEM_SETTINGS_FILE_NAME: &str = "settings.json";
const CLAUDE_MEM_TRANSCRIPT_WATCH_FILE_NAME: &str = "transcript-watch.json";
const CLAUDE_MEM_TRANSCRIPT_WATCH_STATE_FILE_NAME: &str = "transcript-watch-state.json";
const CLAUDE_MEM_PLUGIN_MARKETPLACE_OWNER: &str = "thedotmack";
const CLAUDE_MEM_CODEX_SCHEMA_NAME: &str = "codex";
const CLAUDE_MEM_PRODEX_WATCH_NAME_PREFIX: &str = "prodex-codex-";
const CLAUDE_MEM_CLAUDE_CODE_PATH_SETTING: &str = "CLAUDE_CODE_PATH";
const PRODEX_CLAUDE_MEM_DIR_NAME: &str = "claude-mem";
const PRODEX_CLAUDE_MEM_WRAPPER_NAME: &str = "prodex-claude";
const CLAUDE_MEM_PREFIX: &str = "mem";
const CLAUDE_MEM_FULL_PREFIX: &str = "mem-full";
const CLAUDE_MEM_SUPER_SLIM_PREFIX: &str = "mem-super-slim";
const CLAUDE_MEM_FULL_FLAG: &str = "--mem-full";
const CLAUDE_MEM_SUPER_SLIM_FLAG: &str = "--mem-super-slim";
const RUNTIME_MEM_SUPER_SLIM_PROMPT_SUMMARY_PATHS: &[&str] =
    &["payload.prompt_summary", "payload.metadata.prompt_summary"];
const RUNTIME_MEM_SUPER_SLIM_ARTIFACT_REF_PATHS: &[&str] = &[
    "payload.metadata.artifact_ref",
    "payload.metadata.artifact_id",
    "payload.metadata.artifactId",
    "payload.artifact.reference",
    "payload.artifact.ref",
    "payload.artifact.id",
    "payload.artifact_id",
    "payload.artifactId",
];
const RUNTIME_MEM_SUPER_SLIM_TOOL_SUMMARY_PATHS: &[&str] =
    &["payload.summary", "payload.metadata.summary"];
const RUNTIME_MEM_SUPER_SLIM_TOOL_REF_PATHS: &[&str] = &[
    "payload.metadata.artifact_ref",
    "payload.metadata.artifact_id",
    "payload.metadata.artifactId",
    "payload.artifact.reference",
    "payload.artifact.ref",
    "payload.artifact.id",
    "payload.artifact_id",
    "payload.artifactId",
];
const RUNTIME_MEM_SUPER_SLIM_ASSISTANT_SUMMARY_PATHS: &[&str] = &["payload.summary"];
const RUNTIME_MEM_SUPER_SLIM_SUMMARY_PREFIX_CHAR_LIMIT: usize = 180;
const RUNTIME_MEM_SUPER_SLIM_REFERENCED_SUMMARY_PREFIX_CHAR_LIMIT: usize = 72;
const RUNTIME_MEM_SUPER_SLIM_V2_USER_EVENT_TYPE: &str = "pm2:u";
const RUNTIME_MEM_SUPER_SLIM_V2_ASSISTANT_EVENT_TYPE: &str = "pm2:a";
const RUNTIME_MEM_SUPER_SLIM_V2_TOOL_USE_EVENT_TYPE: &str = "pm2:tu";
const RUNTIME_MEM_SUPER_SLIM_V2_TOOL_RESULT_EVENT_TYPE: &str = "pm2:tr";
const RUNTIME_MEM_SUPER_SLIM_V2_DEFAULT_TOOL_NAME: &str = "tool";
const RUNTIME_MEM_SUPER_SLIM_V2_DEFAULT_TOOL_INPUT: &str = "tool call";
const RUNTIME_MEM_SUPER_SLIM_V2_PREVIOUS_REF_FIELD: &str = "p";
const RUNTIME_MEM_SUPER_SLIM_V2_PREVIOUS_REF_MARKER: &str = "ss:prev";
#[cfg(test)]
const RUNTIME_MEM_SUPER_SLIM_V2_PREVIOUS_REF_MARKER_LEGACY: &str = "ss:ref=prev";
const RUNTIME_MEM_SUPER_SLIM_V2_INTERN_REF_FIELD: &str = "@";
const RUNTIME_MEM_SUPER_SLIM_V2_PREFIX_REF_FIELD: &str = "@p";
const RUNTIME_MEM_SUPER_SLIM_V2_DICTIONARY_EVENT_TYPE: &str = "pm2:d";
const RUNTIME_MEM_SUPER_SLIM_V2_DICTIONARY_REF_PREFIX: &str = "ss:d:";
const RUNTIME_MEM_SUPER_SLIM_V2_DICTIONARY_REF_PREFIX_LEGACY: &str = "ss:dict:";
const RUNTIME_MEM_SUPER_SLIM_V2_DICTIONARY_MODE_EXACT: &str = "e";
const RUNTIME_MEM_SUPER_SLIM_V2_DICTIONARY_MODE_PREFIX: &str = "p";
const RUNTIME_MEM_SUPER_SLIM_V2_DICTIONARY_MODE_EXACT_LEGACY: &str = "exact";
const RUNTIME_MEM_SUPER_SLIM_V2_DICTIONARY_MODE_PREFIX_LEGACY: &str = "prefix";
const RUNTIME_MEM_SUPER_SLIM_OMITTED: &str = "ss:omit";
const RUNTIME_MEM_SUPER_SLIM_PROMPT_OMITTED: &str = "ss:omit=prompt";
const RUNTIME_MEM_SUPER_SLIM_ASSISTANT_OMITTED: &str = "ss:omit=assistant";
const RUNTIME_MEM_SUPER_SLIM_TOOL_OMITTED: &str = "ss:omit=tool";
const RUNTIME_MEM_SUPER_SLIM_V2_MIN_PREFIX_CHARS: usize = 12;
const RUNTIME_MEM_CONVERSATION_ELISION_RECENT_EVENT_WINDOW: usize = 8;
const RUNTIME_MEM_CONVERSATION_ELISION_MIN_CONTENT_BYTES: usize = 384;
const RUNTIME_MEM_CONVERSATION_ELISION_SCAN_CHAR_LIMIT: usize = 8192;
const RUNTIME_MEM_CONVERSATION_ELISION_MAX_FACTS: usize = 4;
const RUNTIME_MEM_CONVERSATION_ELISION_FACT_CHAR_LIMIT: usize = 96;
const RUNTIME_MEM_CONVERSATION_LEDGER_OBJECTIVE_CHAR_LIMIT: usize = 112;
const RUNTIME_MEM_CONVERSATION_LEDGER_SUMMARY_CHAR_LIMIT: usize = 420;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeMemTranscriptMode {
    Slim,
    SuperSlim,
    Full,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeMemSchemaSelectionPolicy {
    Explicit(RuntimeMemTranscriptMode),
    SafeSuperSlimCandidate {
        fallback_mode: RuntimeMemTranscriptMode,
    },
}

pub fn runtime_mem_extract_mode(args: &[OsString]) -> (bool, Vec<OsString>) {
    let (mode, args) = runtime_mem_extract_mode_with_detail(args);
    (mode.is_some(), args)
}

pub fn runtime_mem_super_default_transcript_mode(
    mode: Option<RuntimeMemTranscriptMode>,
) -> Option<RuntimeMemTranscriptMode> {
    match mode {
        Some(RuntimeMemTranscriptMode::Slim) => Some(RuntimeMemTranscriptMode::SuperSlim),
        other => other,
    }
}

pub fn runtime_mem_select_codex_schema_mode_for_event(
    policy: RuntimeMemSchemaSelectionPolicy,
    event: &Value,
) -> RuntimeMemTranscriptMode {
    match policy {
        RuntimeMemSchemaSelectionPolicy::Explicit(mode) => mode,
        RuntimeMemSchemaSelectionPolicy::SafeSuperSlimCandidate { fallback_mode } => {
            runtime_mem_safe_auto_codex_schema_mode_for_event(fallback_mode, event)
        }
    }
}

pub fn runtime_mem_safe_auto_codex_schema_mode_for_event(
    fallback_mode: RuntimeMemTranscriptMode,
    event: &Value,
) -> RuntimeMemTranscriptMode {
    if matches!(fallback_mode, RuntimeMemTranscriptMode::Full) {
        return RuntimeMemTranscriptMode::Full;
    }
    if runtime_mem_event_has_super_slim_prompt_reference(event) {
        RuntimeMemTranscriptMode::SuperSlim
    } else {
        RuntimeMemTranscriptMode::Slim
    }
}

pub fn runtime_mem_codex_schema_for_safe_auto_event(
    fallback_mode: RuntimeMemTranscriptMode,
    event: &Value,
) -> Value {
    runtime_mem_codex_schema_for_mode(runtime_mem_safe_auto_codex_schema_mode_for_event(
        fallback_mode,
        event,
    ))
}

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
    let Some(payload_type) = runtime_mem_lookup_json_path(&shadow, "payload.type")
        .and_then(Value::as_str)
        .map(str::to_string)
    else {
        return shadow;
    };

    match payload_type.as_str() {
        "user_message" => runtime_mem_shadow_user_message(&mut shadow),
        "agent_message" => runtime_mem_shadow_assistant_message(&mut shadow),
        "function_call_output" | "custom_tool_call_output" | "exec_command_output" => {
            runtime_mem_shadow_tool_output(&mut shadow)
        }
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
    content_path: &'static str,
    summary_paths: &'static [&'static str],
    artifact_ref_paths: &'static [&'static str],
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
        let Some(payload_type) =
            runtime_mem_lookup_json_path(event, "payload.type").and_then(Value::as_str)
        else {
            return;
        };
        if !matches!(
            payload_type,
            "function_call" | "custom_tool_call" | "web_search_call" | "exec_command"
        ) {
            return;
        }
        let Some(tool_id) = runtime_mem_first_text_at_paths(event, &["payload.call_id"]) else {
            return;
        };
        let Some(command) = runtime_mem_first_text_at_paths(
            event,
            &["payload.command", "payload.action", "payload.name"],
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
        let content_replacement = runtime_mem_lookup_json_path(event, spec.content_path)
            .and_then(Value::as_str)
            .and_then(|content| {
                let artifact_ref = runtime_mem_first_prodex_artifact_ref_at_paths(
                    event,
                    spec.artifact_ref_paths,
                    content,
                );
                dedupe_state.replacement_for_optional_content(
                    runtime_mem_event_dedupe_id(event, index),
                    content,
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
    let Some(payload_type) =
        runtime_mem_lookup_json_path(event, "payload.type").and_then(Value::as_str)
    else {
        return;
    };
    let (kind, content_path, summary_paths, artifact_ref_paths) = match payload_type {
        "user_message" => (
            RuntimeMemConversationElisionKind::User,
            "payload.message",
            RUNTIME_MEM_SUPER_SLIM_PROMPT_SUMMARY_PATHS,
            RUNTIME_MEM_SUPER_SLIM_ARTIFACT_REF_PATHS,
        ),
        "agent_message" => (
            RuntimeMemConversationElisionKind::Assistant,
            "payload.message",
            RUNTIME_MEM_SUPER_SLIM_ASSISTANT_SUMMARY_PATHS,
            RUNTIME_MEM_SUPER_SLIM_TOOL_REF_PATHS,
        ),
        "function_call_output" | "custom_tool_call_output" | "exec_command_output" => (
            RuntimeMemConversationElisionKind::Tool,
            "payload.output",
            RUNTIME_MEM_SUPER_SLIM_TOOL_SUMMARY_PATHS,
            RUNTIME_MEM_SUPER_SLIM_TOOL_REF_PATHS,
        ),
        _ => return,
    };

    let Some(content) = runtime_mem_lookup_json_path(event, content_path)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|content| content.len() >= RUNTIME_MEM_CONVERSATION_ELISION_MIN_CONTENT_BYTES)
    else {
        return;
    };
    let artifact_ref =
        runtime_mem_first_prodex_artifact_ref_at_paths(event, artifact_ref_paths, content);
    let command = (kind == RuntimeMemConversationElisionKind::Tool)
        .then(|| elision_state.command_for_event(event))
        .flatten();
    let summary =
        runtime_mem_conversation_elision_summary(kind, content, command, artifact_ref.as_deref());
    for path in summary_paths {
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
    let payload_type = runtime_mem_lookup_json_path(event, "payload.type")?.as_str()?;
    if payload_type != "agent_message" {
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
    let payload_type = runtime_mem_lookup_json_path(event, "payload.type")?.as_str()?;
    match payload_type {
        "user_message" => Some(RuntimeMemEventContentSpec {
            content_path: "payload.message",
            summary_paths: RUNTIME_MEM_SUPER_SLIM_PROMPT_SUMMARY_PATHS,
            artifact_ref_paths: RUNTIME_MEM_SUPER_SLIM_ARTIFACT_REF_PATHS,
        }),
        "agent_message" => Some(RuntimeMemEventContentSpec {
            content_path: "payload.message",
            summary_paths: RUNTIME_MEM_SUPER_SLIM_ASSISTANT_SUMMARY_PATHS,
            artifact_ref_paths: RUNTIME_MEM_SUPER_SLIM_TOOL_REF_PATHS,
        }),
        "function_call_output" | "custom_tool_call_output" | "exec_command_output" => {
            Some(RuntimeMemEventContentSpec {
                content_path: "payload.output",
                summary_paths: RUNTIME_MEM_SUPER_SLIM_TOOL_SUMMARY_PATHS,
                artifact_ref_paths: RUNTIME_MEM_SUPER_SLIM_TOOL_REF_PATHS,
            })
        }
        _ => None,
    }
}

fn runtime_mem_event_dedupe_id(event: &Value, index: usize) -> String {
    runtime_mem_first_text_at_paths(event, &["payload.call_id", "payload.id", "id"])
        .unwrap_or_else(|| format!("event[{index}]"))
}

fn runtime_mem_conversation_elision_summary(
    kind: RuntimeMemConversationElisionKind,
    content: &str,
    command: Option<&str>,
    artifact_ref: Option<&str>,
) -> String {
    let mut parts = vec![format!(
        "mem ledger: kind={}; h={}; b={}; t~={}",
        kind.as_str(),
        runtime_mem_content_hash(content),
        content.len(),
        runtime_mem_approx_token_count(content)
    )];

    let objectives = runtime_mem_conversation_objective_facts(kind, content);
    runtime_mem_push_summary_facts(&mut parts, "objective", &objectives);

    let files = runtime_mem_conversation_file_facts(content);
    runtime_mem_push_summary_facts(&mut parts, "files", &files);

    let decisions = runtime_mem_conversation_line_facts(
        content,
        runtime_mem_conversation_line_has_decision_signal,
    );
    runtime_mem_push_summary_facts(&mut parts, "decisions", &decisions);

    let tests = runtime_mem_conversation_test_facts(content, command);
    runtime_mem_push_summary_facts(&mut parts, "tests", &tests);

    let failures =
        runtime_mem_conversation_line_facts(content, runtime_mem_summary_has_critical_signal);
    runtime_mem_push_summary_facts(&mut parts, "open_failures", &failures);

    let artifacts = runtime_mem_conversation_artifact_facts(content, artifact_ref);
    runtime_mem_push_summary_facts(&mut parts, "artifacts", &artifacts);

    runtime_mem_truncate_chars(
        &parts.join("; "),
        RUNTIME_MEM_CONVERSATION_LEDGER_SUMMARY_CHAR_LIMIT,
    )
}

fn runtime_mem_push_summary_facts(parts: &mut Vec<String>, label: &str, facts: &[String]) {
    if facts.is_empty() {
        return;
    }
    parts.push(format!("{label}=[{}]", facts.join(", ")));
}

fn runtime_mem_conversation_objective_facts(
    kind: RuntimeMemConversationElisionKind,
    content: &str,
) -> Vec<String> {
    let mut facts = Vec::new();
    for line in runtime_mem_conversation_scan_text(content).lines() {
        let Some(objective) = runtime_mem_conversation_objective_from_line(kind, line) else {
            continue;
        };
        runtime_mem_push_limited_unique_fact(&mut facts, objective);
        break;
    }
    facts
}

fn runtime_mem_conversation_objective_from_line(
    kind: RuntimeMemConversationElisionKind,
    line: &str,
) -> Option<String> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }
    let lower = line.to_ascii_lowercase();
    let explicit = [
        "objective:",
        "goal:",
        "task:",
        "request:",
        "user asked:",
        "implement ",
        "fix ",
        "add ",
        "update ",
    ]
    .iter()
    .any(|prefix| lower.starts_with(prefix));
    if kind != RuntimeMemConversationElisionKind::User && !explicit {
        return None;
    }
    let normalized = line
        .strip_prefix("Objective:")
        .or_else(|| line.strip_prefix("objective:"))
        .or_else(|| line.strip_prefix("Goal:"))
        .or_else(|| line.strip_prefix("goal:"))
        .or_else(|| line.strip_prefix("Task:"))
        .or_else(|| line.strip_prefix("task:"))
        .or_else(|| line.strip_prefix("Request:"))
        .or_else(|| line.strip_prefix("request:"))
        .or_else(|| line.strip_prefix("User asked:"))
        .or_else(|| line.strip_prefix("user asked:"))
        .map(str::trim)
        .unwrap_or(line);
    Some(runtime_mem_truncate_chars(
        normalized,
        RUNTIME_MEM_CONVERSATION_LEDGER_OBJECTIVE_CHAR_LIMIT,
    ))
}

fn runtime_mem_conversation_file_facts(content: &str) -> Vec<String> {
    let scan = runtime_mem_conversation_scan_text(content);
    let mut facts = Vec::new();
    for raw in scan.split(|ch: char| {
        ch.is_whitespace()
            || matches!(
                ch,
                '"' | '\'' | '`' | '(' | ')' | '[' | ']' | '{' | '}' | '<' | '>' | ',' | ';'
            )
    }) {
        let Some(path) = runtime_mem_conversation_normalize_path_fact(raw) else {
            continue;
        };
        runtime_mem_push_limited_unique_fact(&mut facts, path);
        if facts.len() >= RUNTIME_MEM_CONVERSATION_ELISION_MAX_FACTS {
            break;
        }
    }
    facts
}

fn runtime_mem_conversation_test_facts(content: &str, command: Option<&str>) -> Vec<String> {
    let mut facts = Vec::new();
    if let Some(command) = command
        .and_then(runtime_mem_conversation_normalize_command_fact)
        .filter(|command| runtime_mem_conversation_command_is_test(command))
    {
        runtime_mem_push_limited_unique_fact(&mut facts, command);
    }
    for line in runtime_mem_conversation_scan_text(content).lines() {
        let Some(command) = runtime_mem_conversation_command_from_line(line) else {
            continue;
        };
        if !runtime_mem_conversation_command_is_test(&command) {
            continue;
        }
        runtime_mem_push_limited_unique_fact(&mut facts, command);
        if facts.len() >= RUNTIME_MEM_CONVERSATION_ELISION_MAX_FACTS {
            break;
        }
    }
    facts
}

fn runtime_mem_conversation_command_is_test(command: &str) -> bool {
    let command = command.trim();
    command.starts_with("cargo test")
        || command.starts_with("cargo nextest")
        || command.starts_with("pytest")
        || command.starts_with("python -m pytest")
        || command.starts_with("python3 -m pytest")
        || command.starts_with("npm test")
        || command.starts_with("pnpm test")
        || command.starts_with("yarn test")
        || command.starts_with("go test")
        || command.starts_with("mvn test")
        || command.starts_with("gradle test")
        || command.starts_with("./gradlew test")
        || command.starts_with("just test")
        || command.contains(" cargo test ")
        || command.contains(" pytest ")
}

fn runtime_mem_conversation_artifact_facts(
    content: &str,
    artifact_ref: Option<&str>,
) -> Vec<String> {
    let mut facts = Vec::new();
    if let Some(artifact_ref) = artifact_ref
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        runtime_mem_push_limited_unique_fact(&mut facts, artifact_ref.to_string());
    }
    let aliases = runtime_mem_artifact_aliases_from_text(content);
    for token in runtime_mem_artifact_ref_tokens(&runtime_mem_conversation_scan_text(content)) {
        let Some(artifact_ref) = runtime_mem_parse_artifact_ref_token(token, &aliases) else {
            continue;
        };
        runtime_mem_push_limited_unique_fact(&mut facts, artifact_ref);
        if facts.len() >= RUNTIME_MEM_CONVERSATION_ELISION_MAX_FACTS {
            break;
        }
    }
    facts
}

fn runtime_mem_conversation_line_facts(
    content: &str,
    predicate: impl Fn(&str) -> bool,
) -> Vec<String> {
    let mut facts = Vec::new();
    for line in runtime_mem_conversation_scan_text(content).lines() {
        let line = line.trim();
        if line.is_empty() || !predicate(line) {
            continue;
        }
        runtime_mem_push_limited_unique_fact(
            &mut facts,
            runtime_mem_truncate_chars(line, RUNTIME_MEM_CONVERSATION_ELISION_FACT_CHAR_LIMIT),
        );
        if facts.len() >= RUNTIME_MEM_CONVERSATION_ELISION_MAX_FACTS {
            break;
        }
    }
    facts
}

fn runtime_mem_conversation_scan_text(content: &str) -> String {
    content
        .chars()
        .take(RUNTIME_MEM_CONVERSATION_ELISION_SCAN_CHAR_LIMIT)
        .collect()
}

fn runtime_mem_conversation_normalize_path_fact(raw: &str) -> Option<String> {
    let mut term = raw
        .trim()
        .trim_matches('`')
        .trim_matches('"')
        .trim_matches('\'')
        .trim_matches(|ch: char| matches!(ch, '(' | ')' | '[' | ']' | '{' | '}' | '<' | '>'))
        .trim_end_matches(['.', ',', ';', '!', '?']);
    while let Some((head, tail)) = term.rsplit_once(':') {
        if tail.is_empty() || !tail.chars().all(|ch| ch.is_ascii_digit()) {
            break;
        }
        term = head;
    }
    let term = term.trim_start_matches("./");
    if term.len() < 3 || !runtime_mem_prompt_term_is_path(term) {
        return None;
    }
    Some(runtime_mem_truncate_chars(
        term,
        RUNTIME_MEM_CONVERSATION_ELISION_FACT_CHAR_LIMIT,
    ))
}

fn runtime_mem_conversation_command_from_line(line: &str) -> Option<String> {
    let line = line.trim();
    let command = line
        .strip_prefix('$')
        .or_else(|| line.strip_prefix('>'))
        .map(str::trim)
        .unwrap_or(line);
    if !runtime_mem_conversation_looks_like_command(command) {
        return None;
    }
    runtime_mem_conversation_normalize_command_fact(command)
}

fn runtime_mem_conversation_normalize_command_fact(command: &str) -> Option<String> {
    let command = command.trim();
    if command.is_empty() {
        return None;
    }
    Some(runtime_mem_truncate_chars(
        command,
        RUNTIME_MEM_CONVERSATION_ELISION_FACT_CHAR_LIMIT,
    ))
}

fn runtime_mem_conversation_looks_like_command(command: &str) -> bool {
    [
        "./", "cargo ", "cargo-", "git ", "rg ", "grep ", "npm ", "pnpm ", "yarn ", "pytest",
        "python ", "python3 ", "node ", "prodex ", "codex ", "make ", "just ",
    ]
    .iter()
    .any(|prefix| command.starts_with(prefix))
}

fn runtime_mem_push_limited_unique_fact(facts: &mut Vec<String>, fact: String) {
    let fact = fact.trim();
    if fact.is_empty() || facts.iter().any(|existing| existing == fact) {
        return;
    }
    facts.push(fact.to_string());
}

fn runtime_mem_conversation_line_has_decision_signal(line: &str) -> bool {
    let lower = line.trim().to_ascii_lowercase();
    lower.contains("decision")
        || lower.contains("decided")
        || lower.contains("conclusion")
        || lower.contains("implemented")
        || lower.contains("changed")
        || lower.contains("fixed")
}

fn runtime_mem_lookup_json_path<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    let mut current = value;
    for part in path.split('.') {
        current = current.get(part)?;
    }
    Some(current)
}

fn runtime_mem_shadow_user_message(event: &mut Value) {
    let summary =
        runtime_mem_first_text_at_paths(event, RUNTIME_MEM_SUPER_SLIM_PROMPT_SUMMARY_PATHS)
            .or_else(|| {
                runtime_mem_shadow_summary_for_path(
                    event,
                    "payload.message",
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
    if runtime_mem_lookup_json_path(event, "payload.message").is_some() {
        runtime_mem_set_json_path(
            event,
            "payload.message",
            Value::String(RUNTIME_MEM_SUPER_SLIM_OMITTED.to_string()),
        );
    }
}

fn runtime_mem_shadow_assistant_message(event: &mut Value) {
    if runtime_mem_lookup_json_path(event, "payload.summary").is_none()
        && let Some(summary) = runtime_mem_shadow_summary_for_path(
            event,
            "payload.message",
            "assistant response",
            "message",
        )
    {
        runtime_mem_set_json_path(event, "payload.summary", Value::String(summary));
    }
    if runtime_mem_lookup_json_path(event, "payload.message").is_some() {
        runtime_mem_set_json_path(
            event,
            "payload.message",
            Value::String(RUNTIME_MEM_SUPER_SLIM_OMITTED.to_string()),
        );
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

fn runtime_mem_super_slim_v2_shadow_from_v1_shadow(event: &Value) -> Value {
    let Some(payload_type) =
        runtime_mem_lookup_json_path(event, "payload.type").and_then(Value::as_str)
    else {
        return event.clone();
    };

    match payload_type {
        "user_message" => {
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
        "agent_message" => {
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
        "function_call" | "custom_tool_call" | "web_search_call" | "exec_command" => {
            let mut shadow =
                runtime_mem_short_shadow_event(RUNTIME_MEM_SUPER_SLIM_V2_TOOL_USE_EVENT_TYPE);
            if let Some(tool_id) = runtime_mem_first_text_at_paths(event, &["payload.call_id"]) {
                shadow.insert("i".to_string(), Value::String(tool_id));
            }
            let tool_name =
                runtime_mem_first_text_at_paths(event, &["payload.name", "payload.type"]);
            let tool_input = runtime_mem_first_text_at_paths(
                event,
                &["payload.command", "payload.action", "payload.name"],
            );
            runtime_mem_insert_super_slim_v2_tool_use_fields(&mut shadow, tool_name, tool_input);
            Value::Object(shadow)
        }
        "function_call_output" | "custom_tool_call_output" | "exec_command_output" => {
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
        _ => event.clone(),
    }
}

#[derive(Debug, Default)]
struct RuntimeMemSuperSlimV2ArtifactRefDedupeState {
    previous_emitted_ref: Option<String>,
}

impl RuntimeMemSuperSlimV2ArtifactRefDedupeState {
    fn dedupe_consecutive_event_ref(&mut self, mut event: Value) -> Value {
        let Some(artifact_ref) = runtime_mem_super_slim_v2_emitted_artifact_ref(&event) else {
            self.previous_emitted_ref = None;
            return event;
        };

        if self.previous_emitted_ref.as_deref() == Some(artifact_ref.as_str()) {
            if let Some(object) = event.as_object_mut() {
                object.remove("r");
                object.insert(
                    RUNTIME_MEM_SUPER_SLIM_V2_PREVIOUS_REF_FIELD.to_string(),
                    Value::String(RUNTIME_MEM_SUPER_SLIM_V2_PREVIOUS_REF_MARKER.to_string()),
                );
            }
        } else {
            self.previous_emitted_ref = Some(artifact_ref);
        }

        event
    }
}

#[derive(Debug, Default)]
struct RuntimeMemSuperSlimV2InternState {
    exact_values: HashMap<String, Vec<String>>,
    prefix_values: HashMap<String, Vec<String>>,
    previous_full_values: HashMap<String, Vec<String>>,
    dictionary_values: HashMap<String, HashMap<usize, RuntimeMemSuperSlimV2DictionaryEntry>>,
}

impl RuntimeMemSuperSlimV2InternState {
    fn expand_event(&mut self, mut event: Value) -> Option<Value> {
        let Some(event_type) = runtime_mem_lookup_json_path(&event, "t").and_then(Value::as_str)
        else {
            return Some(event);
        };

        if event_type == RUNTIME_MEM_SUPER_SLIM_V2_DICTIONARY_EVENT_TYPE {
            self.remember_dictionary_event(&event);
            return None;
        }

        let fields: &[&str] = match event_type {
            RUNTIME_MEM_SUPER_SLIM_V2_TOOL_USE_EVENT_TYPE => &["i", "n", "c"],
            RUNTIME_MEM_SUPER_SLIM_V2_TOOL_RESULT_EVENT_TYPE => &["i", "s", "r"],
            RUNTIME_MEM_SUPER_SLIM_V2_USER_EVENT_TYPE => &["s", "r"],
            RUNTIME_MEM_SUPER_SLIM_V2_ASSISTANT_EVENT_TYPE => &["s"],
            _ => &[],
        };
        for field in fields {
            self.expand_field(&mut event, field);
        }
        Some(event)
    }

    fn expand_field(&mut self, event: &mut Value, field: &str) {
        let Some(raw_value) = event.get(field).cloned() else {
            return;
        };
        if let Some(value) = raw_value.as_str() {
            if let Some(resolved) = self.resolve_dictionary_ref(field, value) {
                if let Some(object) = event.as_object_mut() {
                    object.insert(field.to_string(), Value::String(resolved.clone()));
                }
                self.remember_resolved(field, &resolved);
            } else if let Some(resolved) = self.resolve_inline_dictionary_refs(field, value) {
                if let Some(object) = event.as_object_mut() {
                    object.insert(field.to_string(), Value::String(resolved.clone()));
                }
                self.remember_resolved(field, &resolved);
            } else if runtime_mem_super_slim_v2_parse_dictionary_ref(value).is_none() {
                self.remember_resolved(field, value);
            }
            return;
        }

        let Some(value) = self.resolve_marker(field, &raw_value) else {
            return;
        };

        if let Some(object) = event.as_object_mut() {
            object.insert(field.to_string(), Value::String(value.clone()));
        }
        self.remember_resolved(field, &value);
    }

    fn resolve_marker(&self, field: &str, value: &Value) -> Option<String> {
        if let Some(index) = value
            .get(RUNTIME_MEM_SUPER_SLIM_V2_INTERN_REF_FIELD)
            .and_then(Value::as_u64)
            .and_then(|index| usize::try_from(index).ok())
        {
            return self
                .exact_values
                .get(field)
                .and_then(|values| values.get(index))
                .cloned();
        }

        let marker = value
            .get(RUNTIME_MEM_SUPER_SLIM_V2_PREFIX_REF_FIELD)
            .and_then(Value::as_array)?;
        let index = marker
            .first()
            .and_then(Value::as_u64)
            .and_then(|index| usize::try_from(index).ok())?;
        let suffix = marker.get(1).and_then(Value::as_str)?;
        self.prefix_values
            .get(field)
            .and_then(|prefixes| prefixes.get(index))
            .map(|prefix| format!("{prefix}{suffix}"))
    }

    fn remember_dictionary_event(&mut self, event: &Value) {
        let Some(field) = runtime_mem_lookup_json_path(event, "k")
            .and_then(Value::as_str)
            .filter(|field| !field.is_empty())
        else {
            return;
        };
        let Some(index) = runtime_mem_lookup_json_path(event, "i")
            .and_then(runtime_mem_super_slim_v2_dictionary_index)
        else {
            return;
        };
        let Some(mode) = runtime_mem_lookup_json_path(event, "m")
            .and_then(Value::as_str)
            .and_then(RuntimeMemSuperSlimV2DictionaryMode::from_str)
        else {
            return;
        };
        let Some(value) = runtime_mem_lookup_json_path(event, "v")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
        else {
            return;
        };
        self.dictionary_values
            .entry(field.to_string())
            .or_default()
            .insert(
                index,
                RuntimeMemSuperSlimV2DictionaryEntry {
                    mode,
                    value: value.to_string(),
                },
            );
    }

    fn resolve_dictionary_ref(&self, field: &str, value: &str) -> Option<String> {
        let reference = runtime_mem_super_slim_v2_parse_dictionary_ref(value)?;
        if reference.field != field {
            return None;
        }
        let entry = self
            .dictionary_values
            .get(field)
            .and_then(|entries| entries.get(&reference.index))?;
        match (entry.mode, reference.suffix) {
            (RuntimeMemSuperSlimV2DictionaryMode::Exact, None) => Some(entry.value.clone()),
            (RuntimeMemSuperSlimV2DictionaryMode::Prefix, Some(suffix)) => {
                Some(format!("{}{suffix}", entry.value))
            }
            (RuntimeMemSuperSlimV2DictionaryMode::Prefix, None) => Some(entry.value.clone()),
            (RuntimeMemSuperSlimV2DictionaryMode::Exact, Some(_)) => None,
        }
    }

    fn resolve_inline_dictionary_refs(&self, field: &str, value: &str) -> Option<String> {
        let marker_prefixes = [
            format!("{{{RUNTIME_MEM_SUPER_SLIM_V2_DICTIONARY_REF_PREFIX}{field}#"),
            format!("{{{RUNTIME_MEM_SUPER_SLIM_V2_DICTIONARY_REF_PREFIX_LEGACY}{field}#"),
        ];
        let mut output = String::new();
        let mut cursor = 0usize;
        let mut changed = false;
        while let Some((start, marker_len)) = marker_prefixes
            .iter()
            .filter_map(|marker_prefix| {
                value[cursor..]
                    .find(marker_prefix)
                    .map(|relative_start| (cursor + relative_start, marker_prefix.len()))
            })
            .min_by_key(|(start, _)| *start)
        {
            output.push_str(&value[cursor..start]);
            let digits_start = start + marker_len;
            let mut digits_end = digits_start;
            for (offset, ch) in value[digits_start..].char_indices() {
                if !ch.is_ascii_digit() {
                    break;
                }
                digits_end = digits_start + offset + ch.len_utf8();
            }
            if digits_end == digits_start || !value[digits_end..].starts_with('}') {
                output.push_str(&value[start..digits_end]);
                cursor = digits_end;
                continue;
            }
            let Some(index) = value[digits_start..digits_end].parse::<usize>().ok() else {
                output.push_str(&value[start..digits_end]);
                cursor = digits_end;
                continue;
            };
            let Some(entry) = self
                .dictionary_values
                .get(field)
                .and_then(|entries| entries.get(&index))
                .filter(|entry| entry.mode == RuntimeMemSuperSlimV2DictionaryMode::Exact)
            else {
                output.push_str(&value[start..digits_end]);
                cursor = digits_end;
                continue;
            };
            output.push_str(&entry.value);
            cursor = digits_end + 1;
            changed = true;
        }
        if !changed {
            return None;
        }
        output.push_str(&value[cursor..]);
        Some(output)
    }

    fn remember_resolved(&mut self, field: &str, value: &str) {
        self.remember_exact(field, value);
        self.remember_prefixes(field, value);
        self.previous_full_values
            .entry(field.to_string())
            .or_default()
            .push(value.to_string());
    }

    fn remember_exact(&mut self, field: &str, value: &str) {
        let values = self.exact_values.entry(field.to_string()).or_default();
        if !values.iter().any(|candidate| candidate == value) {
            values.push(value.to_string());
        }
    }

    fn remember_prefixes(&mut self, field: &str, value: &str) {
        let Some(previous_values) = self.previous_full_values.get(field) else {
            return;
        };
        let learned = previous_values
            .iter()
            .filter_map(|previous| runtime_mem_common_char_prefix(previous, value))
            .collect::<Vec<_>>();
        if learned.is_empty() {
            return;
        }
        let prefixes = self.prefix_values.entry(field.to_string()).or_default();
        for prefix in learned {
            if !prefixes.iter().any(|candidate| candidate == &prefix) {
                prefixes.push(prefix);
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RuntimeMemSuperSlimV2DictionaryMode {
    Exact,
    Prefix,
}

impl RuntimeMemSuperSlimV2DictionaryMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Exact => RUNTIME_MEM_SUPER_SLIM_V2_DICTIONARY_MODE_EXACT,
            Self::Prefix => RUNTIME_MEM_SUPER_SLIM_V2_DICTIONARY_MODE_PREFIX,
        }
    }

    fn from_str(value: &str) -> Option<Self> {
        match value {
            RUNTIME_MEM_SUPER_SLIM_V2_DICTIONARY_MODE_EXACT
            | RUNTIME_MEM_SUPER_SLIM_V2_DICTIONARY_MODE_EXACT_LEGACY => Some(Self::Exact),
            RUNTIME_MEM_SUPER_SLIM_V2_DICTIONARY_MODE_PREFIX
            | RUNTIME_MEM_SUPER_SLIM_V2_DICTIONARY_MODE_PREFIX_LEGACY => Some(Self::Prefix),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RuntimeMemSuperSlimV2DictionaryEntry {
    mode: RuntimeMemSuperSlimV2DictionaryMode,
    value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RuntimeMemSuperSlimV2DictionaryCandidate {
    field: String,
    mode: RuntimeMemSuperSlimV2DictionaryMode,
    placement: RuntimeMemSuperSlimV2DictionaryPlacement,
    value: String,
    event_indexes: Vec<usize>,
}

impl RuntimeMemSuperSlimV2DictionaryCandidate {
    fn compact_ref(&self, dictionary_index: usize, value: &str) -> Option<String> {
        let base_ref = runtime_mem_super_slim_v2_dictionary_ref(&self.field, dictionary_index);
        match self.mode {
            RuntimeMemSuperSlimV2DictionaryMode::Exact => (value == self.value).then_some(base_ref),
            RuntimeMemSuperSlimV2DictionaryMode::Prefix => value
                .strip_prefix(&self.value)
                .filter(|suffix| !suffix.is_empty())
                .map(|suffix| format!("{base_ref}+{suffix}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RuntimeMemSuperSlimV2DictionaryPlacement {
    WholeField,
    Inline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RuntimeMemSuperSlimV2DictionaryRef<'a> {
    field: &'a str,
    index: usize,
    suffix: Option<&'a str>,
}

fn runtime_mem_super_slim_v2_compact_dictionary_events(mut events: Vec<Value>) -> Vec<Value> {
    while let Some(candidate) = runtime_mem_super_slim_v2_best_dictionary_candidate(&events) {
        let compacted =
            runtime_mem_super_slim_v2_apply_dictionary_candidate(events.clone(), &candidate);
        if runtime_mem_jsonl_events_len(&compacted) >= runtime_mem_jsonl_events_len(&events) {
            break;
        }
        events = compacted;
    }
    events
}

fn runtime_mem_super_slim_v2_best_dictionary_candidate(
    events: &[Value],
) -> Option<RuntimeMemSuperSlimV2DictionaryCandidate> {
    runtime_mem_super_slim_v2_dictionary_candidates(events)
        .into_iter()
        .filter_map(|candidate| {
            runtime_mem_super_slim_v2_candidate_savings(events, &candidate)
                .map(|savings| (candidate, savings))
        })
        .max_by_key(|(candidate, savings)| {
            (
                *savings,
                candidate.event_indexes.len(),
                candidate.value.len(),
            )
        })
        .map(|(candidate, _)| candidate)
}

fn runtime_mem_super_slim_v2_candidate_savings(
    events: &[Value],
    candidate: &RuntimeMemSuperSlimV2DictionaryCandidate,
) -> Option<usize> {
    let dictionary_index =
        runtime_mem_super_slim_v2_next_dictionary_index(events, &candidate.field);
    let dictionary_event_len =
        runtime_mem_json_value_len(&runtime_mem_super_slim_v2_dictionary_event(
            &candidate.field,
            dictionary_index,
            candidate.mode,
            &candidate.value,
        ))
        .saturating_add(1);
    if dictionary_event_len == usize::MAX {
        return None;
    }

    let mut field_savings = 0usize;
    for event_index in &candidate.event_indexes {
        let Some(event) = events.get(*event_index) else {
            continue;
        };
        let Some(value) = event.get(&candidate.field).and_then(Value::as_str) else {
            continue;
        };
        let compacted = match candidate.placement {
            RuntimeMemSuperSlimV2DictionaryPlacement::WholeField => {
                candidate.compact_ref(dictionary_index, value)
            }
            RuntimeMemSuperSlimV2DictionaryPlacement::Inline => {
                let compact_ref = format!(
                    "{{{}}}",
                    runtime_mem_super_slim_v2_dictionary_ref(&candidate.field, dictionary_index)
                );
                let compacted = value.replace(candidate.value.as_str(), &compact_ref);
                (compacted != value).then_some(compacted)
            }
        };
        let Some(compacted) = compacted else {
            continue;
        };

        let original_len = runtime_mem_json_value_len(event).saturating_add(1);
        let mut compacted_event = event.clone();
        let Some(object) = compacted_event.as_object_mut() else {
            continue;
        };
        object.insert(candidate.field.clone(), Value::String(compacted));
        let compacted_len = runtime_mem_json_value_len(&compacted_event).saturating_add(1);
        if compacted_len < original_len {
            field_savings = field_savings.saturating_add(original_len - compacted_len);
        }
    }

    field_savings
        .checked_sub(dictionary_event_len)
        .filter(|savings| *savings > 0)
}

fn runtime_mem_super_slim_v2_dictionary_candidates(
    events: &[Value],
) -> Vec<RuntimeMemSuperSlimV2DictionaryCandidate> {
    let mut candidates = Vec::new();
    for field in ["i", "n", "c", "r", "s"] {
        candidates.extend(runtime_mem_super_slim_v2_exact_dictionary_candidates(
            events, field,
        ));
    }
    for field in ["i", "c", "r", "s"] {
        candidates.extend(runtime_mem_super_slim_v2_prefix_dictionary_candidates(
            events, field,
        ));
    }
    for field in ["c", "r", "s"] {
        candidates.extend(runtime_mem_super_slim_v2_inline_dictionary_candidates(
            events, field,
        ));
    }
    candidates
}

fn runtime_mem_super_slim_v2_exact_dictionary_candidates(
    events: &[Value],
    field: &str,
) -> Vec<RuntimeMemSuperSlimV2DictionaryCandidate> {
    let mut values = HashMap::<String, Vec<usize>>::new();
    for (event_index, value) in runtime_mem_super_slim_v2_field_strings(events, field) {
        values.entry(value).or_default().push(event_index);
    }
    values
        .into_iter()
        .filter(|(_, event_indexes)| event_indexes.len() > 1)
        .map(
            |(value, event_indexes)| RuntimeMemSuperSlimV2DictionaryCandidate {
                field: field.to_string(),
                mode: RuntimeMemSuperSlimV2DictionaryMode::Exact,
                placement: RuntimeMemSuperSlimV2DictionaryPlacement::WholeField,
                value,
                event_indexes,
            },
        )
        .collect()
}

fn runtime_mem_super_slim_v2_prefix_dictionary_candidates(
    events: &[Value],
    field: &str,
) -> Vec<RuntimeMemSuperSlimV2DictionaryCandidate> {
    let values = runtime_mem_super_slim_v2_field_strings(events, field);
    let mut prefixes = Vec::<String>::new();
    for left_index in 0..values.len() {
        for right_index in (left_index + 1)..values.len() {
            let Some(prefix) =
                runtime_mem_common_char_prefix(&values[left_index].1, &values[right_index].1)
            else {
                continue;
            };
            if !prefixes.iter().any(|candidate| candidate == &prefix) {
                prefixes.push(prefix);
            }
        }
    }

    prefixes
        .into_iter()
        .filter_map(|prefix| {
            let event_indexes = values
                .iter()
                .filter(|(_, value)| value.starts_with(&prefix) && value.len() > prefix.len())
                .map(|(event_index, _)| *event_index)
                .collect::<Vec<_>>();
            (event_indexes.len() > 1).then_some(RuntimeMemSuperSlimV2DictionaryCandidate {
                field: field.to_string(),
                mode: RuntimeMemSuperSlimV2DictionaryMode::Prefix,
                placement: RuntimeMemSuperSlimV2DictionaryPlacement::WholeField,
                value: prefix,
                event_indexes,
            })
        })
        .collect()
}

fn runtime_mem_super_slim_v2_inline_dictionary_candidates(
    events: &[Value],
    field: &str,
) -> Vec<RuntimeMemSuperSlimV2DictionaryCandidate> {
    let values = runtime_mem_super_slim_v2_inline_field_strings(events, field);
    let mut term_events = HashMap::<String, Vec<usize>>::new();
    let mut term_occurrences = HashMap::<String, usize>::new();
    for (event_index, value) in &values {
        for term in runtime_mem_super_slim_v2_inline_dictionary_terms(value) {
            let occurrences = value.matches(term.as_str()).count();
            if occurrences == 0 {
                continue;
            }
            let events = term_events.entry(term.clone()).or_default();
            if !events.iter().any(|seen_index| seen_index == event_index) {
                events.push(*event_index);
            }
            *term_occurrences.entry(term).or_default() += occurrences;
        }
    }
    term_events
        .into_iter()
        .filter(|(term, event_indexes)| {
            term_occurrences.get(term).copied().unwrap_or_default() > 1 && !event_indexes.is_empty()
        })
        .map(
            |(value, event_indexes)| RuntimeMemSuperSlimV2DictionaryCandidate {
                field: field.to_string(),
                mode: RuntimeMemSuperSlimV2DictionaryMode::Exact,
                placement: RuntimeMemSuperSlimV2DictionaryPlacement::Inline,
                value,
                event_indexes,
            },
        )
        .collect()
}

fn runtime_mem_super_slim_v2_field_strings(events: &[Value], field: &str) -> Vec<(usize, String)> {
    events
        .iter()
        .enumerate()
        .filter_map(|(event_index, event)| {
            let event_type = runtime_mem_lookup_json_path(event, "t").and_then(Value::as_str)?;
            if !runtime_mem_super_slim_v2_field_can_use_dictionary(event_type, field) {
                return None;
            }
            let value = event.get(field)?.as_str()?;
            if value.is_empty()
                || runtime_mem_super_slim_v2_contains_dictionary_ref_marker(value)
                || runtime_mem_super_slim_v2_parse_dictionary_ref(value).is_some()
            {
                return None;
            }
            Some((event_index, value.to_string()))
        })
        .collect()
}

fn runtime_mem_super_slim_v2_inline_field_strings(
    events: &[Value],
    field: &str,
) -> Vec<(usize, String)> {
    events
        .iter()
        .enumerate()
        .filter_map(|(event_index, event)| {
            let event_type = runtime_mem_lookup_json_path(event, "t").and_then(Value::as_str)?;
            if !runtime_mem_super_slim_v2_field_can_use_dictionary(event_type, field) {
                return None;
            }
            let value = event.get(field)?.as_str()?;
            if value.is_empty() || runtime_mem_super_slim_v2_parse_dictionary_ref(value).is_some() {
                return None;
            }
            Some((event_index, value.to_string()))
        })
        .collect()
}

fn runtime_mem_super_slim_v2_field_can_use_dictionary(event_type: &str, field: &str) -> bool {
    match event_type {
        RUNTIME_MEM_SUPER_SLIM_V2_TOOL_USE_EVENT_TYPE => matches!(field, "i" | "n" | "c"),
        RUNTIME_MEM_SUPER_SLIM_V2_TOOL_RESULT_EVENT_TYPE => matches!(field, "i" | "r" | "s"),
        RUNTIME_MEM_SUPER_SLIM_V2_USER_EVENT_TYPE => matches!(field, "r" | "s"),
        RUNTIME_MEM_SUPER_SLIM_V2_ASSISTANT_EVENT_TYPE => field == "s",
        _ => false,
    }
}

fn runtime_mem_super_slim_v2_inline_dictionary_terms(value: &str) -> Vec<String> {
    let mut terms = Vec::new();
    for token in runtime_mem_super_slim_v2_dictionary_tokens(value) {
        for term in runtime_mem_super_slim_v2_token_dictionary_terms(&token) {
            runtime_mem_push_dictionary_term(&mut terms, term);
        }
    }
    for term in runtime_mem_super_slim_v2_package_terms(value) {
        runtime_mem_push_dictionary_term(&mut terms, term);
    }
    terms
}

fn runtime_mem_super_slim_v2_dictionary_tokens(value: &str) -> Vec<String> {
    value
        .split(|ch: char| {
            ch.is_whitespace()
                || matches!(
                    ch,
                    '"' | '\'' | '`' | '(' | ')' | '[' | ']' | '{' | '}' | '<' | '>' | ',' | ';'
                )
        })
        .filter_map(|token| {
            let token = runtime_mem_super_slim_v2_trim_dictionary_token(token);
            (!token.is_empty()).then(|| token.to_string())
        })
        .collect()
}

fn runtime_mem_super_slim_v2_trim_dictionary_token(token: &str) -> &str {
    token
        .trim_matches(|ch: char| {
            matches!(
                ch,
                '"' | '\'' | '`' | '(' | ')' | '[' | ']' | '{' | '}' | '<' | '>' | ',' | ';'
            )
        })
        .trim_end_matches(['.', ',', ';', '!', '?'])
}

fn runtime_mem_super_slim_v2_token_dictionary_terms(token: &str) -> Vec<String> {
    let mut terms = Vec::new();
    if runtime_mem_super_slim_v2_contains_dictionary_ref_marker(token) {
        return terms;
    }
    if let Some(term) = runtime_mem_super_slim_v2_temp_path_term(token) {
        runtime_mem_push_dictionary_term(&mut terms, term);
    }
    for term in runtime_mem_super_slim_v2_url_terms(token) {
        runtime_mem_push_dictionary_term(&mut terms, term);
    }
    if let Some(term) = runtime_mem_super_slim_v2_identity_term(token) {
        runtime_mem_push_dictionary_term(&mut terms, term);
    }
    if let Some(term) = runtime_mem_super_slim_v2_stack_prefix_term(token) {
        runtime_mem_push_dictionary_term(&mut terms, term);
    }
    terms
}

fn runtime_mem_super_slim_v2_temp_path_term(token: &str) -> Option<String> {
    let token = token.trim_end_matches(':');
    if ![
        "/tmp/",
        "/private/tmp/",
        "/var/tmp/",
        "/var/folders/",
        "/run/user/",
        "/dev/shm/",
    ]
    .iter()
    .any(|prefix| token.starts_with(prefix))
    {
        return None;
    }
    runtime_mem_super_slim_v2_path_prefix_term(token).or_else(|| {
        (token.len() >= RUNTIME_MEM_SUPER_SLIM_V2_MIN_PREFIX_CHARS).then(|| token.to_string())
    })
}

fn runtime_mem_super_slim_v2_path_prefix_term(token: &str) -> Option<String> {
    let slash_index = token.rfind('/')?;
    if slash_index == 0 {
        return None;
    }
    let prefix = &token[..=slash_index];
    (prefix.len() >= RUNTIME_MEM_SUPER_SLIM_V2_MIN_PREFIX_CHARS).then(|| prefix.to_string())
}

fn runtime_mem_super_slim_v2_url_terms(token: &str) -> Vec<String> {
    if !token.starts_with("https://") && !token.starts_with("http://") {
        return Vec::new();
    }
    let mut terms = Vec::new();
    let token = token.trim_end_matches('/');
    if token.len() >= RUNTIME_MEM_SUPER_SLIM_V2_MIN_PREFIX_CHARS {
        runtime_mem_push_dictionary_term(&mut terms, token.to_string());
    }
    let scheme_end = token.find("://").map(|index| index + 3).unwrap_or_default();
    if let Some(path_start) = token[scheme_end..].find('/') {
        let origin_end = scheme_end + path_start;
        let origin = &token[..origin_end];
        if origin.len() >= RUNTIME_MEM_SUPER_SLIM_V2_MIN_PREFIX_CHARS {
            runtime_mem_push_dictionary_term(&mut terms, origin.to_string());
        }
        if let Some(prefix) = runtime_mem_super_slim_v2_path_prefix_term(token) {
            runtime_mem_push_dictionary_term(&mut terms, prefix);
        }
    }
    terms
}

fn runtime_mem_super_slim_v2_identity_term(token: &str) -> Option<String> {
    let token = token
        .trim_end_matches(':')
        .trim_start_matches("profile=")
        .trim_start_matches("account=")
        .trim_start_matches("ref=")
        .trim_start_matches("branch=");
    if token.len() < RUNTIME_MEM_SUPER_SLIM_V2_MIN_PREFIX_CHARS {
        return None;
    }
    let lower = token.to_ascii_lowercase();
    let profile_like = [
        "profile-",
        "profile_",
        "prodex-profile-",
        "codex-profile-",
        "account-",
        "account_",
        "acct-",
        "acct_",
        "org-",
        "org_",
        "proj-",
        "proj_",
        "refs/heads/",
        "refs/remotes/",
        "refs/tags/",
    ]
    .iter()
    .any(|prefix| lower.starts_with(prefix));
    let branch_like = token.contains('/')
        && [
            "origin/",
            "upstream/",
            "feature/",
            "bugfix/",
            "hotfix/",
            "release/",
        ]
        .iter()
        .any(|prefix| lower.starts_with(prefix));
    let git_hash = token.len() >= 12 && token.chars().all(|ch| ch.is_ascii_hexdigit());
    (profile_like || branch_like || git_hash).then(|| token.to_string())
}

fn runtime_mem_super_slim_v2_stack_prefix_term(token: &str) -> Option<String> {
    if token.matches("::").count() < 2 {
        return None;
    }
    let prefix_end = token.rfind("::")? + 2;
    let prefix = &token[..prefix_end];
    (prefix.len() >= RUNTIME_MEM_SUPER_SLIM_V2_MIN_PREFIX_CHARS).then(|| prefix.to_string())
}

fn runtime_mem_super_slim_v2_package_terms(value: &str) -> Vec<String> {
    let tokens = runtime_mem_super_slim_v2_dictionary_tokens(value);
    let mut terms = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        let token = token
            .trim_start_matches("crate=")
            .trim_start_matches("package=")
            .trim_start_matches("pkg=");
        let previous = index.checked_sub(1).and_then(|index| tokens.get(index));
        let package_context = previous.is_some_and(|previous| {
            matches!(
                previous.as_str(),
                "-p" | "--package" | "--pkg" | "--crate" | "crate" | "package" | "pkg"
            )
        }) || value.contains(&format!("crate {token}"))
            || value.contains(&format!("package {token}"));
        let scoped_package = token.starts_with('@') && token.contains('/');
        let crate_like = package_context
            && token.len() >= RUNTIME_MEM_SUPER_SLIM_V2_MIN_PREFIX_CHARS
            && token
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | ':' | '/' | '@'));
        if scoped_package || crate_like {
            runtime_mem_push_dictionary_term(&mut terms, token.to_string());
        }
    }
    terms
}

fn runtime_mem_push_dictionary_term(terms: &mut Vec<String>, term: String) {
    let term = term.trim();
    if term.len() < RUNTIME_MEM_SUPER_SLIM_V2_MIN_PREFIX_CHARS
        || runtime_mem_super_slim_v2_contains_dictionary_ref_marker(term)
        || terms.iter().any(|existing| existing == term)
    {
        return;
    }
    terms.push(term.to_string());
}

fn runtime_mem_super_slim_v2_apply_dictionary_candidate(
    mut events: Vec<Value>,
    candidate: &RuntimeMemSuperSlimV2DictionaryCandidate,
) -> Vec<Value> {
    let Some(insert_at) = candidate.event_indexes.iter().copied().min() else {
        return events;
    };
    let dictionary_index =
        runtime_mem_super_slim_v2_next_dictionary_index(&events, &candidate.field);

    for event_index in &candidate.event_indexes {
        let Some(value) = events
            .get(*event_index)
            .and_then(|event| event.get(&candidate.field))
            .and_then(Value::as_str)
            .map(str::to_string)
        else {
            continue;
        };
        let compacted = match candidate.placement {
            RuntimeMemSuperSlimV2DictionaryPlacement::WholeField => {
                candidate.compact_ref(dictionary_index, &value)
            }
            RuntimeMemSuperSlimV2DictionaryPlacement::Inline => {
                let compact_ref = format!(
                    "{{{}}}",
                    runtime_mem_super_slim_v2_dictionary_ref(&candidate.field, dictionary_index)
                );
                let compacted = value.replace(candidate.value.as_str(), &compact_ref);
                (compacted != value).then_some(compacted)
            }
        };
        if let Some(compacted) = compacted
            && let Some(object) = events.get_mut(*event_index).and_then(Value::as_object_mut)
        {
            object.insert(candidate.field.clone(), Value::String(compacted));
        }
    }

    events.insert(
        insert_at,
        runtime_mem_super_slim_v2_dictionary_event(
            &candidate.field,
            dictionary_index,
            candidate.mode,
            &candidate.value,
        ),
    );
    events
}

fn runtime_mem_super_slim_v2_next_dictionary_index(events: &[Value], field: &str) -> usize {
    events
        .iter()
        .filter(|event| {
            runtime_mem_lookup_json_path(event, "t").and_then(Value::as_str)
                == Some(RUNTIME_MEM_SUPER_SLIM_V2_DICTIONARY_EVENT_TYPE)
        })
        .filter(|event| {
            runtime_mem_lookup_json_path(event, "k").and_then(Value::as_str) == Some(field)
        })
        .filter_map(|event| {
            runtime_mem_lookup_json_path(event, "i")
                .and_then(runtime_mem_super_slim_v2_dictionary_index)
        })
        .max()
        .map_or(0, |index| index + 1)
}

fn runtime_mem_super_slim_v2_dictionary_event(
    field: &str,
    dictionary_index: usize,
    mode: RuntimeMemSuperSlimV2DictionaryMode,
    value: &str,
) -> Value {
    let mut event = runtime_mem_short_shadow_event(RUNTIME_MEM_SUPER_SLIM_V2_DICTIONARY_EVENT_TYPE);
    let index = dictionary_index.to_string();
    event.insert("k".to_string(), Value::String(field.to_string()));
    event.insert("i".to_string(), Value::String(index.clone()));
    event.insert("m".to_string(), Value::String(mode.as_str().to_string()));
    event.insert("v".to_string(), Value::String(value.to_string()));
    event.insert(
        "s".to_string(),
        Value::String(format!("dict {field}#{index} {}={value}", mode.as_str())),
    );
    Value::Object(event)
}

fn runtime_mem_super_slim_v2_dictionary_ref(field: &str, dictionary_index: usize) -> String {
    format!("{RUNTIME_MEM_SUPER_SLIM_V2_DICTIONARY_REF_PREFIX}{field}#{dictionary_index}")
}

fn runtime_mem_super_slim_v2_parse_dictionary_ref(
    value: &str,
) -> Option<RuntimeMemSuperSlimV2DictionaryRef<'_>> {
    let rest = value
        .strip_prefix(RUNTIME_MEM_SUPER_SLIM_V2_DICTIONARY_REF_PREFIX)
        .or_else(|| value.strip_prefix(RUNTIME_MEM_SUPER_SLIM_V2_DICTIONARY_REF_PREFIX_LEGACY))?;
    let (field, index_and_suffix) = rest.split_once('#')?;
    if field.is_empty() || !field.chars().all(|ch| ch.is_ascii_alphanumeric()) {
        return None;
    }
    let (index, suffix) = index_and_suffix
        .split_once('+')
        .map_or((index_and_suffix, None), |(index, suffix)| {
            (index, Some(suffix))
        });
    let index = index.parse::<usize>().ok()?;
    Some(RuntimeMemSuperSlimV2DictionaryRef {
        field,
        index,
        suffix,
    })
}

fn runtime_mem_super_slim_v2_contains_dictionary_ref_marker(value: &str) -> bool {
    value.contains(RUNTIME_MEM_SUPER_SLIM_V2_DICTIONARY_REF_PREFIX)
        || value.contains(RUNTIME_MEM_SUPER_SLIM_V2_DICTIONARY_REF_PREFIX_LEGACY)
}

fn runtime_mem_super_slim_v2_dictionary_index(value: &Value) -> Option<usize> {
    value
        .as_str()
        .and_then(|value| value.parse::<usize>().ok())
        .or_else(|| value.as_u64().and_then(|value| usize::try_from(value).ok()))
}

fn runtime_mem_jsonl_events_len(events: &[Value]) -> usize {
    events
        .iter()
        .map(|event| runtime_mem_json_value_len(event).saturating_add(1))
        .sum()
}

fn runtime_mem_super_slim_v2_emitted_artifact_ref(event: &Value) -> Option<String> {
    let event_type = runtime_mem_lookup_json_path(event, "t")?.as_str()?;
    if !matches!(
        event_type,
        RUNTIME_MEM_SUPER_SLIM_V2_USER_EVENT_TYPE
            | RUNTIME_MEM_SUPER_SLIM_V2_TOOL_RESULT_EVENT_TYPE
    ) {
        return None;
    }
    runtime_mem_lookup_json_path(event, "r")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn runtime_mem_common_char_prefix(left: &str, right: &str) -> Option<String> {
    let mut prefix_len = 0usize;
    for ((left_index, left_char), (right_index, right_char)) in
        left.char_indices().zip(right.char_indices())
    {
        if left_char != right_char {
            break;
        }
        prefix_len = left_index + left_char.len_utf8();
        if left_index != right_index {
            break;
        }
    }
    if prefix_len < RUNTIME_MEM_SUPER_SLIM_V2_MIN_PREFIX_CHARS {
        return None;
    }
    Some(left[..prefix_len].to_string())
}

fn runtime_mem_json_value_len(value: &Value) -> usize {
    serde_json::to_string(value).map_or(usize::MAX, |value| value.len())
}

fn runtime_mem_value_is_text_or_v2_intern_marker(value: &Value) -> bool {
    runtime_mem_value_is_text(value)
        || value
            .get(RUNTIME_MEM_SUPER_SLIM_V2_INTERN_REF_FIELD)
            .and_then(Value::as_u64)
            .is_some()
        || value
            .get(RUNTIME_MEM_SUPER_SLIM_V2_PREFIX_REF_FIELD)
            .and_then(Value::as_array)
            .is_some_and(|marker| {
                marker.first().and_then(Value::as_u64).is_some()
                    && marker.get(1).and_then(Value::as_str).is_some()
            })
}

fn runtime_mem_insert_super_slim_v2_tool_use_fields(
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

fn runtime_mem_super_slim_v2_should_keep_summary(
    summary: Option<&str>,
    artifact_ref: Option<&str>,
) -> bool {
    let Some(summary) = summary else {
        return false;
    };
    artifact_ref.is_none() || runtime_mem_summary_has_critical_signal(summary)
}

fn runtime_mem_summary_has_critical_signal(summary: &str) -> bool {
    let lower = summary.to_ascii_lowercase();
    [
        "error",
        "failed",
        "failure",
        "panic",
        "exception",
        "traceback",
        "fatal",
        "cannot",
        "denied",
        "timed out",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

fn runtime_mem_short_shadow_event(event_type: &str) -> serde_json::Map<String, Value> {
    let mut shadow = serde_json::Map::new();
    shadow.insert("t".to_string(), Value::String(event_type.to_string()));
    shadow
}

fn runtime_mem_shadow_summary_for_path(
    event: &Value,
    path: &str,
    label: &str,
    omitted_name: &str,
) -> Option<String> {
    let text = runtime_mem_lookup_json_path(event, path)?.as_str()?;
    let artifact_ref = runtime_mem_extract_artifact_marker(event);
    Some(runtime_mem_shadow_summary_from_text(
        text,
        label,
        omitted_name,
        artifact_ref.as_deref(),
    ))
}

fn runtime_mem_shadow_summary_from_text(
    text: &str,
    label: &str,
    omitted_name: &str,
    artifact_ref: Option<&str>,
) -> String {
    let label = runtime_mem_shadow_summary_label(label);
    let prefix_limit = runtime_mem_shadow_summary_prefix_char_limit(artifact_ref);
    let first_line = runtime_mem_first_useful_line(text)
        .map(|line| runtime_mem_truncate_chars(line, prefix_limit))
        .unwrap_or_else(|| "(empty)".to_string());
    let ref_part = artifact_ref
        .filter(|value| !value.trim().is_empty())
        .map(|value| format!(" ref={value}"))
        .unwrap_or_default();
    format!(
        "{label}: {first_line} [b={} t~={} omit={omitted_name}{ref_part}]",
        text.len(),
        runtime_mem_approx_token_count(text),
    )
}

fn runtime_mem_shadow_summary_label(label: &str) -> &str {
    match label {
        "user prompt" => "u",
        "assistant response" => "a",
        "tool output" => "tool",
        _ => label,
    }
}

fn runtime_mem_shadow_summary_prefix_char_limit(artifact_ref: Option<&str>) -> usize {
    if artifact_ref.is_some_and(|value| runtime_mem_normalize_prodex_artifact_ref(value).is_some())
    {
        RUNTIME_MEM_SUPER_SLIM_REFERENCED_SUMMARY_PREFIX_CHAR_LIMIT
    } else {
        RUNTIME_MEM_SUPER_SLIM_SUMMARY_PREFIX_CHAR_LIMIT
    }
}

fn runtime_mem_first_useful_line(text: &str) -> Option<&str> {
    text.lines().map(str::trim).find(|line| !line.is_empty())
}

fn runtime_mem_truncate_chars(text: &str, max_chars: usize) -> String {
    let mut chars = text.chars();
    let truncated = chars.by_ref().take(max_chars).collect::<String>();
    if chars.next().is_some() {
        format!("{truncated}...")
    } else {
        truncated
    }
}

fn runtime_mem_approx_token_count(text: &str) -> usize {
    text.split_whitespace().count()
}

fn runtime_mem_first_text_at_paths(event: &Value, paths: &[&str]) -> Option<String> {
    paths.iter().find_map(|path| {
        runtime_mem_lookup_json_path(event, path)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(str::to_string)
    })
}

fn runtime_mem_set_json_path(value: &mut Value, path: &str, new_value: Value) {
    let mut current = value;
    let mut parts = path.split('.').peekable();
    while let Some(part) = parts.next() {
        if parts.peek().is_none() {
            if let Value::Object(object) = current {
                object.insert(part.to_string(), new_value);
            }
            return;
        }
        if !current.is_object() {
            *current = serde_json::json!({});
        }
        let object = current
            .as_object_mut()
            .expect("json path container should be object");
        current = object
            .entry(part.to_string())
            .or_insert_with(|| serde_json::json!({}));
    }
}

fn runtime_mem_value_is_text(value: &Value) -> bool {
    value.as_str().is_some_and(|text| !text.trim().is_empty())
}

fn runtime_mem_value_contains_artifact_marker(value: &Value) -> bool {
    if runtime_mem_extract_artifact_marker(value).is_some() {
        return true;
    }
    runtime_mem_value_contains_text(value, "prodex smart context artifact")
}

fn runtime_mem_value_contains_text(value: &Value, needle: &str) -> bool {
    match value {
        Value::String(text) => text.contains(needle),
        Value::Array(values) => values
            .iter()
            .any(|value| runtime_mem_value_contains_text(value, needle)),
        Value::Object(object) => object
            .values()
            .any(|value| runtime_mem_value_contains_text(value, needle)),
        _ => false,
    }
}

#[cfg(test)]
#[path = "../tests/src/lib.rs"]
mod tests;

#[cfg(test)]
mod compact_v2_runtime_memory_tests {
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

    #[test]
    fn super_slim_v2_interns_repeated_tool_names_when_smaller() {
        let tool_name = "very_long_custom_repo_tool_name_for_runtime_mem_schema_native_dictionary";
        let events = (0..8)
            .map(|index| {
                serde_json::json!({
                "payload": {
                    "type": "custom_tool_call",
                    "call_id": format!("call-tool-name-{index}"),
                    "name": tool_name,
                    "action": format!("action {index}")
                }
                })
            })
            .collect::<Vec<_>>();

        let base_shadows = v2_shadow_events_without_dictionary(events.iter());
        let shadows = runtime_mem_super_slim_v2_shadow_codex_events(events.iter());

        assert!(
            runtime_mem_jsonl_events_len(&shadows) < runtime_mem_jsonl_events_len(&base_shadows)
        );
        assert!(v2_dictionary_events(&shadows).iter().any(|event| {
            event.get("k").and_then(Value::as_str) == Some("n")
                && event.get("m").and_then(Value::as_str)
                    == Some(RUNTIME_MEM_SUPER_SLIM_V2_DICTIONARY_MODE_EXACT)
                && event.get("v").and_then(Value::as_str) == Some(tool_name)
        }));
        assert!(v2_tool_use_events(&shadows).iter().any(|event| {
            event
                .get("n")
                .and_then(Value::as_str)
                .is_some_and(|value| value.starts_with("ss:d:n#"))
        }));
        assert_v2_raw_events_schema_addressable(&shadows);
        assert_v2_compact_fields_are_strings(&shadows);

        let expanded = expanded_non_dictionary_events(shadows.clone());
        let fields = v2_schema_fields("prodex-v2-tool-use");
        for event in v2_tool_use_events(&expanded) {
            assert_eq!(
                resolve_v2_schema_string(&fields["toolName"], event).as_deref(),
                Some(tool_name)
            );
        }
    }

    #[test]
    fn super_slim_v2_interns_command_and_repo_path_prefixes_when_smaller() {
        let command_prefix =
            "cargo test -q -p prodex-runtime-mem --lib compact_v2_runtime_memory_tests::";
        let repo_prefix = "/workspace/prodex/crates/prodex-runtime-mem/src/runtime/schema/native/";
        let mut events = Vec::new();
        for name in [
            "alpha", "beta", "gamma", "delta", "epsilon", "zeta", "eta", "theta",
        ] {
            events.push(serde_json::json!({
                "payload": {
                    "type": "exec_command",
                    "call_id": format!("call-cmd-{name}"),
                    "command": format!("{command_prefix}{name}")
                }
            }));
        }
        for name in [
            "lib.rs",
            "tests.rs",
            "schema.rs",
            "dictionary.rs",
            "prefix.rs",
            "call_id.rs",
            "shadow.rs",
            "expand.rs",
        ] {
            events.push(serde_json::json!({
                "payload": {
                    "type": "user_message",
                    "message": format!("path ref {name}"),
                    "metadata": {
                        "artifact_ref": format!("{repo_prefix}{name}")
                    }
                }
            }));
        }

        let base_shadows = v2_shadow_events_without_dictionary(events.iter());
        let shadows = runtime_mem_super_slim_v2_shadow_codex_events(events.iter());

        assert!(
            runtime_mem_jsonl_events_len(&shadows) < runtime_mem_jsonl_events_len(&base_shadows)
        );
        assert!(v2_dictionary_events(&shadows).iter().any(|event| {
            event.get("k").and_then(Value::as_str) == Some("c")
                && event.get("m").and_then(Value::as_str)
                    == Some(RUNTIME_MEM_SUPER_SLIM_V2_DICTIONARY_MODE_PREFIX)
        }));
        assert!(v2_dictionary_events(&shadows).iter().any(|event| {
            event.get("k").and_then(Value::as_str) == Some("r")
                && event.get("m").and_then(Value::as_str)
                    == Some(RUNTIME_MEM_SUPER_SLIM_V2_DICTIONARY_MODE_PREFIX)
        }));
        assert!(v2_tool_use_events(&shadows).iter().any(|event| {
            event
                .get("c")
                .and_then(Value::as_str)
                .is_some_and(|value| value.starts_with("ss:d:c#"))
        }));
        assert!(v2_user_events(&shadows).iter().any(|event| {
            event
                .get("r")
                .and_then(Value::as_str)
                .is_some_and(|value| value.starts_with("ss:d:r#"))
        }));
        assert_v2_raw_events_schema_addressable(&shadows);
        assert_v2_compact_fields_are_strings(&shadows);

        let expanded = expanded_non_dictionary_events(shadows);
        let tool_fields = v2_schema_fields("prodex-v2-tool-use");
        let user_fields = v2_schema_fields("prodex-v2-user-message");
        let tool_inputs = v2_tool_use_events(&expanded)
            .iter()
            .filter_map(|event| resolve_v2_schema_string(&tool_fields["toolInput"], event))
            .collect::<Vec<_>>();
        let user_prompts = v2_user_events(&expanded)
            .iter()
            .filter_map(|event| resolve_v2_schema_string(&user_fields["prompt"], event))
            .collect::<Vec<_>>();
        assert!(tool_inputs.contains(&format!("{command_prefix}theta")));
        assert!(user_prompts.contains(&format!("{repo_prefix}expand.rs")));
    }

    #[test]
    fn super_slim_v2_interns_call_id_prefixes_when_smaller() {
        let call_id_prefix = "call_01HF97R8Y9_prodex_runtime_mem_schema_native_dictionary_";
        let events = [
            "alpha", "beta", "gamma", "delta", "epsilon", "zeta", "eta", "theta",
        ]
        .into_iter()
        .map(|suffix| {
            serde_json::json!({
                "payload": {
                    "type": "function_call",
                    "call_id": format!("{call_id_prefix}{suffix}"),
                    "name": "tool"
                }
            })
        })
        .collect::<Vec<_>>();

        let base_shadows = v2_shadow_events_without_dictionary(events.iter());
        let shadows = runtime_mem_super_slim_v2_shadow_codex_events(events.iter());

        assert!(
            runtime_mem_jsonl_events_len(&shadows) < runtime_mem_jsonl_events_len(&base_shadows)
        );
        assert!(v2_dictionary_events(&shadows).iter().any(|event| {
            event.get("k").and_then(Value::as_str) == Some("i")
                && event.get("m").and_then(Value::as_str)
                    == Some(RUNTIME_MEM_SUPER_SLIM_V2_DICTIONARY_MODE_PREFIX)
        }));
        assert!(v2_tool_use_events(&shadows).iter().any(|event| {
            event
                .get("i")
                .and_then(Value::as_str)
                .is_some_and(|value| value.starts_with("ss:d:i#"))
        }));
        assert_v2_raw_events_schema_addressable(&shadows);
        assert_v2_compact_fields_are_strings(&shadows);

        let expanded = expanded_non_dictionary_events(shadows);
        let fields = v2_schema_fields("prodex-v2-tool-use");
        let tool_ids = v2_tool_use_events(&expanded)
            .iter()
            .filter_map(|event| resolve_v2_schema_string(&fields["toolId"], event))
            .collect::<Vec<_>>();
        assert!(tool_ids.contains(&format!("{call_id_prefix}theta")));
    }

    #[test]
    fn super_slim_v2_interns_exact_repeated_tool_ids_when_smaller() {
        let call_id = "call_exact_prodex_runtime_mem_dictionary_repeated_identifier_0123456789";
        let mut events = Vec::new();
        for index in 0..6 {
            events.push(serde_json::json!({
                "payload": {
                    "type": "function_call",
                    "call_id": call_id,
                    "name": "tool",
                    "arguments": format!("input {index}")
                }
            }));
            events.push(serde_json::json!({
                "payload": {
                    "type": "function_call_output",
                    "call_id": call_id,
                    "output": format!("output {index}")
                }
            }));
        }

        let base_shadows = v2_shadow_events_without_dictionary(events.iter());
        let shadows = runtime_mem_super_slim_v2_shadow_codex_events(events.iter());

        assert!(
            runtime_mem_jsonl_events_len(&shadows) < runtime_mem_jsonl_events_len(&base_shadows)
        );
        assert!(v2_dictionary_events(&shadows).iter().any(|event| {
            event.get("k").and_then(Value::as_str) == Some("i")
                && event.get("m").and_then(Value::as_str)
                    == Some(RUNTIME_MEM_SUPER_SLIM_V2_DICTIONARY_MODE_EXACT)
                && event.get("v").and_then(Value::as_str) == Some(call_id)
        }));
        assert_v2_raw_events_schema_addressable(&shadows);
        assert_v2_compact_fields_are_strings(&shadows);

        let expanded = expanded_non_dictionary_events(shadows);
        for event in v2_tool_events_with_ids(&expanded) {
            assert_eq!(event.get("i").and_then(Value::as_str), Some(call_id));
        }
    }

    #[test]
    fn super_slim_v2_interns_exact_repeated_summaries_when_smaller() {
        let summary = "user: exact repeated runtime memory summary retained through expansion";
        let events = (0..8)
            .map(|index| {
                serde_json::json!({
                    "payload": {
                        "type": "user_message",
                        "id": format!("user-{index}"),
                        "message": format!("full user prompt {index} {}", "detail ".repeat(40)),
                        "metadata": {
                            "prompt_summary": summary
                        }
                    }
                })
            })
            .collect::<Vec<_>>();

        let base_shadows = v2_shadow_events_without_dictionary(events.iter());
        let shadows = runtime_mem_super_slim_v2_shadow_codex_events(events.iter());

        assert!(
            runtime_mem_jsonl_events_len(&shadows) < runtime_mem_jsonl_events_len(&base_shadows)
        );
        assert!(v2_dictionary_events(&shadows).iter().any(|event| {
            event.get("k").and_then(Value::as_str) == Some("s")
                && event.get("m").and_then(Value::as_str)
                    == Some(RUNTIME_MEM_SUPER_SLIM_V2_DICTIONARY_MODE_EXACT)
                && event.get("v").and_then(Value::as_str) == Some(summary)
        }));
        assert_v2_raw_events_schema_addressable(&shadows);
        assert_v2_compact_fields_are_strings(&shadows);

        let expanded = expanded_non_dictionary_events(shadows);
        let fields = v2_schema_fields("prodex-v2-user-message");
        for event in v2_user_events(&expanded) {
            assert_eq!(
                resolve_v2_schema_string(&fields["prompt"], event).as_deref(),
                Some(summary)
            );
        }
    }

    #[test]
    fn super_slim_v2_dictionary_skips_compaction_when_not_smaller() {
        let events = [
            serde_json::json!({
                "payload": {
                    "type": "custom_tool_call",
                    "call_id": "call-a",
                    "name": "sh",
                    "action": "a"
                }
            }),
            serde_json::json!({
                "payload": {
                    "type": "custom_tool_call",
                    "call_id": "call-b",
                    "name": "sh",
                    "action": "b"
                }
            }),
        ];

        let base_shadows = v2_shadow_events_without_dictionary(events.iter());
        let shadows = runtime_mem_super_slim_v2_shadow_codex_events(events.iter());

        assert_eq!(shadows, base_shadows);
        assert!(v2_dictionary_events(&shadows).is_empty());
        assert_v2_raw_events_schema_addressable(&shadows);
    }

    #[test]
    fn super_slim_v2_dictionary_candidate_savings_matches_applied_jsonl_delta() {
        let summary = "user: repeated summary for candidate savings";
        let events = (0..6)
            .map(|index| {
                serde_json::json!({
                    "payload": {
                        "type": "user_message",
                        "message": format!("full user prompt {index} {}", "detail ".repeat(16)),
                        "metadata": {
                            "prompt_summary": summary
                        }
                    }
                })
            })
            .collect::<Vec<_>>();
        let base_shadows = v2_shadow_events_without_dictionary(events.iter());
        let candidate = runtime_mem_super_slim_v2_dictionary_candidates(&base_shadows)
            .into_iter()
            .find(|candidate| {
                candidate.field == "s"
                    && candidate.mode == RuntimeMemSuperSlimV2DictionaryMode::Exact
                    && candidate.value == summary
            })
            .expect("expected repeated summary dictionary candidate");

        let estimated = runtime_mem_super_slim_v2_candidate_savings(&base_shadows, &candidate)
            .expect("candidate should shrink JSONL");
        let compacted =
            runtime_mem_super_slim_v2_apply_dictionary_candidate(base_shadows.clone(), &candidate);
        let actual =
            runtime_mem_jsonl_events_len(&base_shadows) - runtime_mem_jsonl_events_len(&compacted);

        assert_eq!(estimated, actual);
    }

    #[test]
    fn super_slim_v2_intern_expansion_preserves_legacy_explicit_events() {
        let legacy = serde_json::json!({
            "t": RUNTIME_MEM_SUPER_SLIM_V2_TOOL_USE_EVENT_TYPE,
            "i": "call-legacy-explicit",
            "n": "exec_command",
            "c": "cargo test -q -p prodex-runtime-mem --lib"
        });

        let expanded = runtime_mem_super_slim_v2_expand_interned_events([legacy]);
        let fields = v2_schema_fields("prodex-v2-tool-use");
        assert_eq!(
            resolve_v2_schema_string(&fields["toolId"], &expanded[0]).as_deref(),
            Some("call-legacy-explicit")
        );
        assert_eq!(
            resolve_v2_schema_string(&fields["toolName"], &expanded[0]).as_deref(),
            Some("exec_command")
        );
        assert_eq!(
            resolve_v2_schema_string(&fields["toolInput"], &expanded[0]).as_deref(),
            Some("cargo test -q -p prodex-runtime-mem --lib")
        );
    }

    #[test]
    fn super_slim_v2_intern_expansion_accepts_legacy_dictionary_refs() {
        let events = [
            serde_json::json!({
                "t": RUNTIME_MEM_SUPER_SLIM_V2_DICTIONARY_EVENT_TYPE,
                "k": "n",
                "i": "0",
                "m": RUNTIME_MEM_SUPER_SLIM_V2_DICTIONARY_MODE_EXACT_LEGACY,
                "v": "legacy_tool"
            }),
            serde_json::json!({
                "t": RUNTIME_MEM_SUPER_SLIM_V2_DICTIONARY_EVENT_TYPE,
                "k": "c",
                "i": "0",
                "m": RUNTIME_MEM_SUPER_SLIM_V2_DICTIONARY_MODE_EXACT_LEGACY,
                "v": "legacy inline command"
            }),
            serde_json::json!({
                "t": RUNTIME_MEM_SUPER_SLIM_V2_DICTIONARY_EVENT_TYPE,
                "k": "r",
                "i": "0",
                "m": RUNTIME_MEM_SUPER_SLIM_V2_DICTIONARY_MODE_PREFIX_LEGACY,
                "v": "psc:legacy-prefix-"
            }),
            serde_json::json!({
                "t": RUNTIME_MEM_SUPER_SLIM_V2_TOOL_USE_EVENT_TYPE,
                "i": "call-legacy-dict",
                "n": "ss:dict:n#0",
                "c": "run {ss:dict:c#0}"
            }),
            serde_json::json!({
                "t": RUNTIME_MEM_SUPER_SLIM_V2_USER_EVENT_TYPE,
                "r": "ss:dict:r#0+tail"
            }),
        ];

        let expanded = runtime_mem_super_slim_v2_expand_interned_events(events);
        let tool_fields = v2_schema_fields("prodex-v2-tool-use");
        let user_fields = v2_schema_fields("prodex-v2-user-message");

        assert_eq!(
            resolve_v2_schema_string(&tool_fields["toolName"], &expanded[0]).as_deref(),
            Some("legacy_tool")
        );
        assert_eq!(
            resolve_v2_schema_string(&tool_fields["toolInput"], &expanded[0]).as_deref(),
            Some("run legacy inline command")
        );
        assert_eq!(
            resolve_v2_schema_string(&user_fields["prompt"], &expanded[1]).as_deref(),
            Some("psc:legacy-prefix-tail")
        );
    }

    fn v2_shadow_events_without_dictionary<'a>(
        events: impl IntoIterator<Item = &'a Value>,
    ) -> Vec<Value> {
        let mut ref_dedupe_state = RuntimeMemSuperSlimV2ArtifactRefDedupeState::default();
        runtime_mem_super_slim_shadow_codex_events(events)
            .iter()
            .map(runtime_mem_super_slim_v2_shadow_from_v1_shadow)
            .map(|event| ref_dedupe_state.dedupe_consecutive_event_ref(event))
            .collect()
    }

    fn v2_dictionary_events(events: &[Value]) -> Vec<&Value> {
        events
            .iter()
            .filter(|event| {
                runtime_mem_lookup_json_path(event, "t").and_then(Value::as_str)
                    == Some(RUNTIME_MEM_SUPER_SLIM_V2_DICTIONARY_EVENT_TYPE)
            })
            .collect()
    }

    fn v2_tool_use_events(events: &[Value]) -> Vec<&Value> {
        v2_events_of_type(events, RUNTIME_MEM_SUPER_SLIM_V2_TOOL_USE_EVENT_TYPE)
    }

    fn v2_tool_events_with_ids(events: &[Value]) -> Vec<&Value> {
        events
            .iter()
            .filter(|event| {
                matches!(
                    runtime_mem_lookup_json_path(event, "t").and_then(Value::as_str),
                    Some(RUNTIME_MEM_SUPER_SLIM_V2_TOOL_USE_EVENT_TYPE)
                        | Some(RUNTIME_MEM_SUPER_SLIM_V2_TOOL_RESULT_EVENT_TYPE)
                )
            })
            .collect()
    }

    fn v2_user_events(events: &[Value]) -> Vec<&Value> {
        v2_events_of_type(events, RUNTIME_MEM_SUPER_SLIM_V2_USER_EVENT_TYPE)
    }

    fn v2_events_of_type<'a>(events: &'a [Value], event_type: &str) -> Vec<&'a Value> {
        events
            .iter()
            .filter(|event| {
                runtime_mem_lookup_json_path(event, "t").and_then(Value::as_str) == Some(event_type)
            })
            .collect()
    }

    fn expanded_non_dictionary_events(events: Vec<Value>) -> Vec<Value> {
        runtime_mem_super_slim_v2_expand_interned_events(events)
            .into_iter()
            .filter(|event| {
                runtime_mem_lookup_json_path(event, "t").and_then(Value::as_str)
                    != Some(RUNTIME_MEM_SUPER_SLIM_V2_DICTIONARY_EVENT_TYPE)
            })
            .collect()
    }

    fn assert_v2_raw_events_schema_addressable(events: &[Value]) {
        for event in events {
            match runtime_mem_lookup_json_path(event, "t").and_then(Value::as_str) {
                Some(RUNTIME_MEM_SUPER_SLIM_V2_USER_EVENT_TYPE) => {
                    assert_v2_schema_fields_are_strings(
                        "prodex-v2-user-message",
                        event,
                        &["prompt"],
                    );
                }
                Some(RUNTIME_MEM_SUPER_SLIM_V2_ASSISTANT_EVENT_TYPE) => {
                    assert_v2_schema_fields_are_strings(
                        "prodex-v2-assistant-message",
                        event,
                        &["message"],
                    );
                }
                Some(RUNTIME_MEM_SUPER_SLIM_V2_TOOL_USE_EVENT_TYPE) => {
                    assert_v2_schema_fields_are_strings(
                        "prodex-v2-tool-use",
                        event,
                        &["toolId", "toolName", "toolInput"],
                    );
                }
                Some(RUNTIME_MEM_SUPER_SLIM_V2_TOOL_RESULT_EVENT_TYPE) => {
                    assert_v2_schema_fields_are_strings(
                        "prodex-v2-tool-result",
                        event,
                        &["toolId", "toolResponse"],
                    );
                }
                Some(RUNTIME_MEM_SUPER_SLIM_V2_DICTIONARY_EVENT_TYPE) => {
                    assert_v2_schema_fields_are_strings(
                        "prodex-v2-dictionary-entry",
                        event,
                        &[
                            "dictionary",
                            "dictionaryKey",
                            "dictionaryIndex",
                            "dictionaryMode",
                            "dictionaryValue",
                        ],
                    );
                }
                _ => {}
            }
        }
    }

    fn assert_v2_schema_fields_are_strings(event_name: &str, event: &Value, field_names: &[&str]) {
        let fields = v2_schema_fields(event_name);
        for field_name in field_names {
            let value = resolve_v2_schema_string(&fields[*field_name], event)
                .unwrap_or_else(|| panic!("{event_name}.{field_name} should resolve: {event}"));
            assert!(
                !value.trim().is_empty(),
                "{event_name}.{field_name} should be meaningful: {event}"
            );
        }
    }

    fn assert_v2_compact_fields_are_strings(events: &[Value]) {
        for event in events {
            for field in [
                "i",
                "n",
                "c",
                "r",
                "s",
                "p",
                "k",
                "m",
                "v",
                RUNTIME_MEM_SUPER_SLIM_V2_PREVIOUS_REF_FIELD,
            ] {
                if let Some(value) = event.get(field) {
                    assert!(value.is_string(), "{field} should stay string: {event}");
                }
            }
        }
    }

    fn v2_schema_fields(event_name: &str) -> Value {
        runtime_mem_super_slim_codex_schema()
            .get("events")
            .and_then(Value::as_array)
            .and_then(|events| {
                events
                    .iter()
                    .find(|event| event.get("name").and_then(Value::as_str) == Some(event_name))
            })
            .and_then(|event| event.get("fields"))
            .cloned()
            .expect("v2 schema fields should exist")
    }

    fn resolve_v2_schema_string(spec: &Value, entry: &Value) -> Option<String> {
        resolve_v2_schema_field(spec, entry).and_then(|value| match value {
            Value::String(value) => Some(value),
            _ => None,
        })
    }

    fn resolve_v2_schema_field(spec: &Value, entry: &Value) -> Option<Value> {
        if let Some(path) = spec.as_str() {
            return runtime_mem_lookup_json_path(entry, path).cloned();
        }
        if let Some(value) = spec.get("value") {
            return Some(value.clone());
        }
        if let Some(coalesce) = spec.get("coalesce").and_then(Value::as_array) {
            for candidate in coalesce {
                if let Some(value) = resolve_v2_schema_field(candidate, entry)
                    && !value.as_str().is_some_and(str::is_empty)
                {
                    return Some(value);
                }
            }
        }
        None
    }
}
