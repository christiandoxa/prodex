use serde_json::Value;
use std::ffi::OsString;

mod artifact_refs;
mod json_path;
mod recall_selection;
mod schema;
mod text;
mod watch;

pub use artifact_refs::runtime_mem_content_hash;
pub(crate) use artifact_refs::{
    runtime_mem_artifact_aliases_from_text, runtime_mem_artifact_recall_summary,
    runtime_mem_artifact_ref_tokens, runtime_mem_duplicate_recall_summary,
    runtime_mem_extract_artifact_marker, runtime_mem_first_artifact_ref_text_at_paths,
    runtime_mem_first_prodex_artifact_ref_at_paths, runtime_mem_normalize_prodex_artifact_ref,
    runtime_mem_parse_artifact_ref_token, runtime_mem_prodex_artifact_ref,
};
pub(crate) use json_path::{
    runtime_mem_first_text_at_paths, runtime_mem_first_text_path_at_paths,
    runtime_mem_lookup_json_path, runtime_mem_set_json_path,
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
pub(crate) use text::{
    runtime_mem_approx_token_count, runtime_mem_first_useful_line, runtime_mem_truncate_chars,
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
const RUNTIME_MEM_MESSAGE_TEXT_PATHS: &[&str] = &[
    "payload.message",
    "payload.content[0].text",
    "payload.content[1].text",
    "payload.content[2].text",
    "payload.content[3].text",
    "payload.content[4].text",
];
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

mod dictionary;
mod shadow;

#[cfg(test)]
pub(crate) use dictionary::{
    RuntimeMemSuperSlimV2ArtifactRefDedupeState, RuntimeMemSuperSlimV2DictionaryMode,
    runtime_mem_jsonl_events_len, runtime_mem_super_slim_v2_apply_dictionary_candidate,
    runtime_mem_super_slim_v2_candidate_savings, runtime_mem_super_slim_v2_dictionary_candidates,
};
pub use shadow::{
    runtime_mem_event_has_super_slim_prompt_reference, runtime_mem_extract_mode_with_detail,
    runtime_mem_super_slim_shadow_codex_event, runtime_mem_super_slim_shadow_codex_events,
    runtime_mem_super_slim_v2_expand_interned_events, runtime_mem_super_slim_v2_shadow_codex_event,
    runtime_mem_super_slim_v2_shadow_codex_events,
};
#[cfg(test)]
pub(crate) use shadow::{
    runtime_mem_super_slim_v2_shadow_from_v1_shadow, runtime_mem_value_contains_artifact_marker,
};

#[cfg(test)]
#[path = "../tests/src/lib.rs"]
mod tests;

#[cfg(test)]
#[path = "../tests/src/compact_v2_runtime_memory.rs"]
mod compact_v2_runtime_memory_tests;
