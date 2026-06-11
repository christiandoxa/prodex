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

mod args;
mod conversation;
mod dedupe;
mod elision;
mod event;
mod summary;
mod v1;
mod v2;
mod value;

pub use self::args::runtime_mem_extract_mode_with_detail;
use self::conversation::runtime_mem_conversation_elision_summary;
use self::dedupe::{
    RuntimeMemEventDedupeState, runtime_mem_super_slim_shadow_codex_event_with_dedupe,
};
pub(crate) use self::elision::{
    RuntimeMemConversationElisionKind, RuntimeMemConversationElisionState,
    runtime_mem_elide_old_conversation_event,
};
pub(crate) use self::event::{
    RuntimeMemCodexPayloadKind, RuntimeMemEventContentSpec, runtime_mem_codex_payload_kind,
    runtime_mem_event_content_spec,
};
use self::summary::runtime_mem_summary_has_critical_signal;
pub use self::v1::{
    runtime_mem_event_has_super_slim_prompt_reference, runtime_mem_super_slim_shadow_codex_event,
};
pub(crate) use self::v2::{
    runtime_mem_short_shadow_event, runtime_mem_super_slim_v2_shadow_from_v1_shadow,
};
pub(crate) use self::value::{
    runtime_mem_value_contains_artifact_marker, runtime_mem_value_is_text,
};

pub fn runtime_mem_super_slim_shadow_codex_events<'a>(
    events: impl IntoIterator<Item = &'a Value>,
) -> Vec<Value> {
    let mut dedupe_state = RuntimeMemEventDedupeState::default();
    let events = events.into_iter();
    let (lower_bound, upper_bound) = events.size_hint();
    if upper_bound == Some(lower_bound) {
        let mut elision_state = RuntimeMemConversationElisionState::new(lower_bound);
        return events
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
            .collect();
    }

    let events = events.collect::<Vec<_>>();
    let mut elision_state = RuntimeMemConversationElisionState::new(events.len());
    events
        .into_iter()
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
