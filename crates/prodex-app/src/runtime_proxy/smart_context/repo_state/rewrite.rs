use super::*;

pub(super) struct RuntimeSmartContextRepoStateMicroCacheTextInput<'a> {
    pub(super) text: &'a mut String,
    pub(super) command: Option<&'a str>,
    pub(super) cache_before: &'a RuntimeSmartContextRepoStateFacts,
    pub(super) cache_after: &'a mut RuntimeSmartContextRepoStateFacts,
    pub(super) store: &'a mut RuntimeSmartContextArtifactStore,
    pub(super) request_id: u64,
    pub(super) allow_rewrite: bool,
    pub(super) stats: &'a mut RuntimeSmartContextTransformStats,
}

pub(super) fn runtime_smart_context_apply_repo_state_micro_cache_to_text(
    input: RuntimeSmartContextRepoStateMicroCacheTextInput<'_>,
) -> bool {
    let RuntimeSmartContextRepoStateMicroCacheTextInput {
        text,
        command,
        cache_before,
        cache_after,
        store,
        request_id,
        allow_rewrite,
        stats,
    } = input;
    let observation = runtime_smart_context_repo_state_text_observation(text, command);
    if observation.facts.is_empty() {
        return false;
    }
    let relation = observation.facts.relation_to(cache_before);
    cache_after.merge_from(&observation.facts);
    if !allow_rewrite
        || relation != RuntimeSmartContextRepoStateFactRelation::Repeated
        || observation.spans.is_empty()
    {
        return false;
    }

    let needs_artifact = text.len() >= SMART_CONTEXT_REPO_STATE_ARTIFACT_MIN_BYTES;
    let artifact_ref = needs_artifact.then(|| {
        let id = runtime_proxy_crate::smart_context_hash_text(text);
        runtime_smart_context_artifact_ref(&id)
    });
    let marker =
        runtime_smart_context_repo_state_repeat_marker(&observation.facts, artifact_ref.as_deref());
    let Some(candidate) =
        runtime_smart_context_repo_state_replaced_text(text, &observation.spans, &marker)
    else {
        return false;
    };
    if candidate.len() >= text.len()
        || prodex_context::critical_signal_self_check(text, &candidate).has_loss()
    {
        return false;
    }

    if needs_artifact {
        let existing_artifact = store.artifact_ref_for_exact_text(text.as_str());
        if existing_artifact.is_none() && store.insert_text(request_id, text.as_str()).is_none() {
            return false;
        }
        if existing_artifact.is_none() {
            stats.artifacts_stored = stats.artifacts_stored.saturating_add(1);
        }
    }

    *text = candidate;
    stats.repo_state_facts = stats.repo_state_facts.saturating_add(1);
    true
}

fn runtime_smart_context_repo_state_replaced_text(
    text: &str,
    spans: &[RuntimeSmartContextRepoStateLineSpan],
    marker: &str,
) -> Option<String> {
    let spans = runtime_smart_context_repo_state_merge_spans(spans.to_vec());
    if spans.is_empty() {
        return None;
    }
    let lines = text.split('\n').collect::<Vec<_>>();
    let mut rendered = Vec::<String>::new();
    let mut line_index = 0usize;
    let mut span_index = 0usize;
    while line_index < lines.len() {
        if let Some(span) = spans.get(span_index)
            && line_index == span.start
        {
            rendered.push(marker.to_string());
            line_index = span.end.min(lines.len());
            span_index = span_index.saturating_add(1);
            continue;
        }
        rendered.push(lines[line_index].to_string());
        line_index = line_index.saturating_add(1);
    }
    Some(rendered.join("\n"))
}

fn runtime_smart_context_repo_state_repeat_marker(
    facts: &RuntimeSmartContextRepoStateFacts,
    artifact_ref: Option<&str>,
) -> String {
    let mut parts = vec![
        format!("{SMART_CONTEXT_REPO_STATE_MARKER_PREFIX}repeat"),
        format!(
            "h={}",
            runtime_smart_context_repo_state_facts_short_hash(facts)
        ),
    ];
    if let Some(branch) = facts.branch.as_deref() {
        parts.push(format!("branch={branch}"));
    }
    if let Some(dirty_files) = facts.dirty_files.as_ref() {
        parts.push(format!("dirty={}", dirty_files.len()));
    }
    if let Some(recent_changed_files) = facts.recent_changed_files.as_ref() {
        parts.push(format!("recent={}", recent_changed_files.len()));
    }
    if let Some(package_managers) = facts.package_managers.as_ref() {
        parts.push(format!(
            "pm={}",
            package_managers
                .iter()
                .cloned()
                .collect::<Vec<_>>()
                .join(",")
        ));
    }
    if let Some(commands) = facts.main_test_commands.as_ref() {
        parts.push(format!(
            "test='{}'",
            runtime_smart_context_repo_state_compact_commands(commands)
        ));
    }
    if let Some(artifact_ref) = artifact_ref {
        parts.push(format!("ref={artifact_ref}"));
    }
    parts.join(" ")
}
