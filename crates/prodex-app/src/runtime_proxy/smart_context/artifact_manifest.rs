use super::*;

mod aliases;

pub(in crate::runtime_proxy::smart_context) use aliases::*;

pub(super) fn runtime_smart_context_artifact_marker_line(
    kind: &str,
    artifact: &runtime_proxy_crate::SmartContextArtifactRef,
) -> String {
    let reference = runtime_smart_context_artifact_ref(&artifact.id);
    let kind = match kind {
        "artifact" => "art",
        other => other,
    };
    format!(
        "psc {kind} {reference} b={} lines=#Lx-Ly",
        artifact.byte_len
    )
}

pub(super) fn runtime_smart_context_artifact_line_ref(
    id: &str,
    start: usize,
    end: usize,
) -> String {
    format!("{}#L{start}-L{end}", runtime_smart_context_artifact_ref(id))
}

pub(super) fn runtime_smart_context_artifact_ref(id: &str) -> String {
    let short_id = id.strip_prefix("sc:").unwrap_or(id);
    format!("{SMART_CONTEXT_SHORT_ARTIFACT_REF_PREFIX}{short_id}")
}

pub(super) fn runtime_smart_context_artifact_id_valid(id: &str) -> bool {
    id.strip_prefix("sc:")
        .is_some_and(|rest| !rest.is_empty() && rest.chars().all(|ch| ch.is_ascii_hexdigit()))
}

pub(super) fn runtime_smart_context_text_is_generated_summary_or_manifest(text: &str) -> bool {
    let trimmed = text.trim_start();
    trimmed.starts_with("prodex-sc artifact ")
        || trimmed.starts_with("prodex-sc repeat ")
        || trimmed.starts_with("psc art ")
        || trimmed.starts_with("psc rep ")
        || trimmed.starts_with("psc repeat ")
        || trimmed.starts_with("psc co ")
        || trimmed.starts_with("psc cmdout ")
        || trimmed.starts_with(SMART_CONTEXT_REPO_STATE_MARKER_PREFIX)
        || trimmed.starts_with(SMART_CONTEXT_REPO_STATE_MARKER_PREFIX_LEGACY)
        || trimmed.starts_with("psc m ")
        || trimmed.starts_with("psc manifest ")
}

pub(super) fn runtime_smart_context_append_artifact_manifest_delta_if_useful(
    value: &mut serde_json::Value,
    state: &mut RuntimeSmartContextProxyState,
    stats: &RuntimeSmartContextTransformStats,
    intent_signals: &RuntimeSmartContextIntentSignals,
) -> bool {
    if !runtime_smart_context_artifact_manifest_useful(stats) {
        return false;
    }
    if !runtime_smart_context_artifact_manifest_delta_eligible(state, intent_signals) {
        return false;
    }
    if runtime_smart_context_explicit_artifact_refs_fully_resolved_without_manifest_request(
        state,
        intent_signals,
    ) {
        return false;
    }
    let mut entries = state
        .artifacts
        .artifact_manifest_entries(SMART_CONTEXT_ARTIFACT_MANIFEST_MAX_ENTRIES);
    runtime_smart_context_filter_manifest_entries(value, intent_signals, &mut entries);
    let ids = entries
        .iter()
        .map(|entry| entry.id.clone())
        .collect::<BTreeSet<_>>();
    if ids.is_empty() || ids == state.last_artifact_manifest_ids {
        return false;
    }
    let detailed_manifest = runtime_smart_context_manifest_requested(intent_signals);
    let unchanged_count = ids.intersection(&state.last_artifact_manifest_ids).count();
    let manifest_entries = if detailed_manifest {
        entries
    } else {
        entries
            .into_iter()
            .filter(|entry| !state.last_artifact_manifest_ids.contains(&entry.id))
            .collect::<Vec<_>>()
    };
    let Some(manifest) = runtime_smart_context_artifact_manifest_delta_from_entries(
        manifest_entries,
        detailed_manifest,
        unchanged_count,
    ) else {
        return false;
    };
    if !runtime_smart_context_append_input_manifest(value, manifest) {
        return false;
    }
    state.last_artifact_manifest_ids = ids;
    state.last_artifact_manifest_emitted_at = Some(Instant::now());
    true
}

pub(super) fn runtime_smart_context_explicit_artifact_refs_fully_resolved_without_manifest_request(
    state: &RuntimeSmartContextProxyState,
    intent_signals: &RuntimeSmartContextIntentSignals,
) -> bool {
    !runtime_smart_context_manifest_requested(intent_signals)
        && !intent_signals.artifact_refs.is_empty()
        && intent_signals
            .artifact_refs
            .iter()
            .all(|reference| state.artifacts.contains(&reference.id))
}

pub(super) fn runtime_smart_context_artifact_manifest_delta_eligible(
    state: &RuntimeSmartContextProxyState,
    intent_signals: &RuntimeSmartContextIntentSignals,
) -> bool {
    if runtime_smart_context_manifest_requested(intent_signals) {
        return true;
    }
    if intent_signals.artifact_refs.is_empty()
        && runtime_smart_context_selective_rehydrate_terms_empty(&intent_signals.semantic_terms)
        && intent_signals.command_kind_hints.is_empty()
    {
        return false;
    }
    state
        .last_artifact_manifest_emitted_at
        .is_none_or(|emitted_at| {
            emitted_at.elapsed()
                >= Duration::from_millis(SMART_CONTEXT_ARTIFACT_MANIFEST_COOLDOWN_MS)
        })
}

#[cfg(test)]
pub(super) fn runtime_smart_context_append_artifact_manifest_if_useful(
    value: &mut serde_json::Value,
    store: &RuntimeSmartContextArtifactStore,
    stats: &RuntimeSmartContextTransformStats,
) -> bool {
    if !runtime_smart_context_artifact_manifest_useful(stats) {
        return false;
    }
    let Some(manifest) =
        runtime_smart_context_artifact_manifest_for_value(value, store, &Default::default())
    else {
        return false;
    };
    runtime_smart_context_append_input_manifest(value, manifest)
}

pub(super) fn runtime_smart_context_artifact_manifest_useful(
    stats: &RuntimeSmartContextTransformStats,
) -> bool {
    stats.artifacts_stored > 0
        || stats.tool_outputs_condensed > 0
        || stats.cross_turn_duplicate_texts > 0
        || stats.repeat_tool_output_refs > 0
}

#[cfg(test)]
pub(super) fn runtime_smart_context_artifact_manifest(
    store: &RuntimeSmartContextArtifactStore,
) -> Option<String> {
    let entries = store.artifact_manifest_entries(SMART_CONTEXT_ARTIFACT_MANIFEST_MAX_ENTRIES);
    runtime_smart_context_artifact_manifest_from_entries(entries, true)
}

#[cfg(test)]
pub(super) fn runtime_smart_context_artifact_manifest_for_value(
    value: &serde_json::Value,
    store: &RuntimeSmartContextArtifactStore,
    intent_signals: &RuntimeSmartContextIntentSignals,
) -> Option<String> {
    let mut entries = store.artifact_manifest_entries(SMART_CONTEXT_ARTIFACT_MANIFEST_MAX_ENTRIES);
    runtime_smart_context_filter_manifest_entries(value, intent_signals, &mut entries);
    runtime_smart_context_artifact_manifest_from_entries(
        entries,
        runtime_smart_context_manifest_requested(intent_signals),
    )
}

pub(super) fn runtime_smart_context_filter_manifest_entries(
    value: &serde_json::Value,
    intent_signals: &RuntimeSmartContextIntentSignals,
    entries: &mut Vec<RuntimeSmartContextArtifactManifestEntry>,
) {
    if !runtime_smart_context_manifest_requested(intent_signals) {
        let visible_ids = runtime_smart_context_collect_artifact_refs(value)
            .into_iter()
            .map(|reference| reference.id)
            .collect::<BTreeSet<_>>();
        entries.retain(|entry| !visible_ids.contains(&entry.id));
    }
    runtime_smart_context_sort_manifest_entries_for_intent(entries, intent_signals);
}

pub(super) fn runtime_smart_context_manifest_requested(
    intent_signals: &RuntimeSmartContextIntentSignals,
) -> bool {
    intent_signals.intent_terms.iter().any(|term| {
        matches!(
            term.as_str(),
            "artifact"
                | "artifacts"
                | "manifest"
                | "reference"
                | "references"
                | "refs"
                | "rehydrate"
        )
    })
}

pub(super) fn runtime_smart_context_sort_manifest_entries_for_intent(
    entries: &mut Vec<RuntimeSmartContextArtifactManifestEntry>,
    intent_signals: &RuntimeSmartContextIntentSignals,
) {
    if runtime_smart_context_selective_rehydrate_terms_empty(&intent_signals.semantic_terms) {
        return;
    }
    let original = std::mem::take(entries);
    let (mut matching, rest): (Vec<_>, Vec<_>) = original.into_iter().partition(|entry| {
        runtime_smart_context_manifest_entry_matches_intent(entry, intent_signals)
    });
    matching.extend(rest);
    *entries = matching;
}

pub(super) fn runtime_smart_context_manifest_entry_matches_intent(
    entry: &RuntimeSmartContextArtifactManifestEntry,
    intent_signals: &RuntimeSmartContextIntentSignals,
) -> bool {
    entry.file_location_range_count > 0 && !intent_signals.semantic_terms.file_paths.is_empty()
        || entry.error_range_count > 0 && !intent_signals.semantic_terms.error_codes.is_empty()
        || entry.test_failure_range_count > 0
            && !intent_signals.semantic_terms.test_symbols.is_empty()
        || entry.diff_hunk_range_count > 0 && !intent_signals.semantic_terms.diff_hunks.is_empty()
}

#[cfg(test)]
pub(super) fn runtime_smart_context_artifact_manifest_from_entries(
    entries: Vec<RuntimeSmartContextArtifactManifestEntry>,
    detailed: bool,
) -> Option<String> {
    runtime_smart_context_artifact_manifest_delta_from_entries(entries, detailed, 0)
}

pub(super) fn runtime_smart_context_artifact_manifest_delta_from_entries(
    entries: Vec<RuntimeSmartContextArtifactManifestEntry>,
    detailed: bool,
    unchanged_count: usize,
) -> Option<String> {
    if entries.is_empty() {
        if unchanged_count == 0 {
            return None;
        }
        return Some(format!("psc m refs same={unchanged_count}"));
    }

    let mut header = "psc m refs".to_string();
    if unchanged_count > 0 {
        header.push_str(&format!(" same={unchanged_count}"));
    }
    let mut lines = vec![header];
    let mut rendered_len = lines[0].len();
    for entry in entries {
        let semantic_count = entry
            .file_location_range_count
            .saturating_add(entry.diff_hunk_range_count)
            .saturating_add(entry.test_failure_range_count)
            .saturating_add(entry.error_range_count);
        let reference = runtime_smart_context_artifact_ref(&entry.id);
        let mut parts = vec![format!("- {reference}"), format!("b={}", entry.byte_len)];
        if detailed && entry.critical_range_count > 0 {
            parts.push(format!("cr={}", entry.critical_range_count));
        }
        if detailed && semantic_count > 0 {
            parts.push(format!("sr={semantic_count}"));
        }
        if detailed && entry.file_location_range_count > 0 {
            parts.push(format!("f={}", entry.file_location_range_count));
        }
        if detailed && entry.diff_hunk_range_count > 0 {
            parts.push(format!("d={}", entry.diff_hunk_range_count));
        }
        if detailed && entry.test_failure_range_count > 0 {
            parts.push(format!("t={}", entry.test_failure_range_count));
        }
        if detailed && entry.error_range_count > 0 {
            parts.push(format!("e={}", entry.error_range_count));
        }
        if detailed && let Some(kind) = entry.command_kind.as_deref() {
            parts.push(format!("k={kind}"));
        }
        let line = parts.join(" ");
        if rendered_len.saturating_add(line.len()).saturating_add(1)
            > SMART_CONTEXT_ARTIFACT_MANIFEST_MAX_CHARS
        {
            lines.push("[psc m trunc]".to_string());
            break;
        }
        rendered_len = rendered_len.saturating_add(line.len()).saturating_add(1);
        lines.push(line);
    }
    Some(lines.join("\n"))
}

pub(super) fn runtime_smart_context_append_input_manifest(
    value: &mut serde_json::Value,
    manifest: String,
) -> bool {
    let Some(input) = value
        .get_mut("input")
        .and_then(serde_json::Value::as_array_mut)
    else {
        return false;
    };
    input.push(serde_json::json!({
        "type": "message",
        "role": "user",
        "content": manifest,
    }));
    true
}
