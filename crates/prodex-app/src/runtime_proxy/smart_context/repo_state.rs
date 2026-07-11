use super::{
    RUNTIME_SMART_CONTEXT_STATIC_PROMPT_FIELDS, RuntimeSmartContextArtifactStore,
    RuntimeSmartContextProxyState, RuntimeSmartContextTransformStats,
    SMART_CONTEXT_REPO_STATE_ARTIFACT_MIN_BYTES, SMART_CONTEXT_REPO_STATE_HASH_CHARS,
    SMART_CONTEXT_REPO_STATE_MARKER_PREFIX, SMART_CONTEXT_REPO_STATE_MARKER_PREFIX_LEGACY,
    SMART_CONTEXT_REPO_STATE_MAX_FACTS_PER_SET, SMART_CONTEXT_REPO_STATE_TEST_COMMAND_MAX_CHARS,
    runtime_smart_context_artifact_ref, runtime_smart_context_static_role_is_prompt_prefix,
    runtime_smart_context_text_contains_any, runtime_smart_context_tool_call_id,
    runtime_smart_context_tool_call_metadata_by_call_id,
};
mod facts;
mod parser;
mod rewrite;

pub(super) use facts::RuntimeSmartContextRepoStateFacts;
use facts::*;
use parser::*;
use rewrite::*;

pub(super) fn runtime_smart_context_apply_repo_state_micro_cache(
    value: &mut serde_json::Value,
    state: &mut RuntimeSmartContextProxyState,
    request_id: u64,
    exactness_guard: &runtime_proxy_crate::SmartContextExactnessGuard,
    allow_rewrite: bool,
    stats: &mut RuntimeSmartContextTransformStats,
) -> bool {
    if exactness_guard.decision != runtime_proxy_crate::SmartContextExactnessDecision::Allow {
        return false;
    }
    let cache_before = state.repo_state_facts.clone();
    let mut cache_after = cache_before.clone();
    let mut rewritten = false;
    let tool_call_metadata = value
        .get("input")
        .and_then(serde_json::Value::as_array)
        .map(|input| runtime_smart_context_tool_call_metadata_by_call_id(input))
        .unwrap_or_default();

    for metadata in tool_call_metadata.values() {
        if let Some(command) = metadata.command.as_deref()
            && let Some(test_command) = runtime_smart_context_repo_state_test_command(command)
        {
            let mut facts = RuntimeSmartContextRepoStateFacts::default();
            runtime_smart_context_repo_state_insert_fact(
                &mut facts.main_test_commands,
                test_command,
            );
            cache_after.merge_from(&facts);
        }
    }

    let Some(object) = value.as_object_mut() else {
        if state.repo_state_facts != cache_after {
            state.repo_state_facts = cache_after;
        }
        return false;
    };
    for key in RUNTIME_SMART_CONTEXT_STATIC_PROMPT_FIELDS {
        if let Some(serde_json::Value::String(text)) = object.get_mut(key) {
            rewritten |= runtime_smart_context_apply_repo_state_micro_cache_to_text(
                RuntimeSmartContextRepoStateMicroCacheTextInput {
                    text,
                    command: None,
                    cache_before: &cache_before,
                    cache_after: &mut cache_after,
                    store: &mut state.artifacts,
                    request_id,
                    allow_rewrite,
                    stats,
                },
            );
        }
    }

    let Some(input) = object
        .get_mut("input")
        .and_then(serde_json::Value::as_array_mut)
    else {
        if state.repo_state_facts != cache_after {
            state.repo_state_facts = cache_after;
        }
        return rewritten;
    };

    for item in input {
        let Some(item_object) = item.as_object_mut() else {
            continue;
        };
        let item_type = item_object
            .get("type")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string();
        let role = item_object
            .get("role")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string();
        if runtime_smart_context_static_role_is_prompt_prefix(&role) {
            for field in ["content", "input_text"] {
                if let Some(serde_json::Value::String(text)) = item_object.get_mut(field) {
                    rewritten |= runtime_smart_context_apply_repo_state_micro_cache_to_text(
                        RuntimeSmartContextRepoStateMicroCacheTextInput {
                            text,
                            command: None,
                            cache_before: &cache_before,
                            cache_after: &mut cache_after,
                            store: &mut state.artifacts,
                            request_id,
                            allow_rewrite,
                            stats,
                        },
                    );
                }
            }
            continue;
        }
        if !item_type.ends_with("_call_output") {
            continue;
        }
        let command = runtime_smart_context_tool_call_id(item_object)
            .and_then(|call_id| tool_call_metadata.get(call_id))
            .and_then(|metadata| metadata.command.as_deref())
            .map(str::to_string);
        for field in ["output", "content"] {
            if let Some(serde_json::Value::String(text)) = item_object.get_mut(field) {
                rewritten |= runtime_smart_context_apply_repo_state_micro_cache_to_text(
                    RuntimeSmartContextRepoStateMicroCacheTextInput {
                        text,
                        command: command.as_deref(),
                        cache_before: &cache_before,
                        cache_after: &mut cache_after,
                        store: &mut state.artifacts,
                        request_id,
                        allow_rewrite,
                        stats,
                    },
                );
            }
        }
    }

    if state.repo_state_facts != cache_after {
        state.repo_state_facts = cache_after;
    }
    rewritten
}
