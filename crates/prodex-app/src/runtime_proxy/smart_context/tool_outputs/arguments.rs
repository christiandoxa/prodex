use super::*;

pub(super) fn runtime_smart_context_tool_argument_text(
    value: &serde_json::Value,
) -> Option<String> {
    match value {
        serde_json::Value::String(text) if !text.trim().is_empty() => Some(text.clone()),
        serde_json::Value::Object(_) | serde_json::Value::Array(_) => {
            serde_json::to_string(value).ok()
        }
        _ => None,
    }
}

pub(super) fn runtime_smart_context_tool_argument_replacement(
    artifact: &runtime_proxy_crate::SmartContextArtifactRef,
    text: &str,
    tier: runtime_proxy_crate::SmartContextTokenBudgetTier,
    already_stored: bool,
    previous_arguments: &[RuntimeSmartContextToolArgumentCandidate],
) -> String {
    if let Some(previous) = previous_arguments
        .iter()
        .find(|previous| previous.text == text)
    {
        return runtime_smart_context_tool_argument_repeat_summary(
            artifact,
            Some(&previous.artifact),
        );
    }
    if already_stored {
        return runtime_smart_context_tool_argument_repeat_summary(artifact, None);
    }
    if let Some(delta) = runtime_smart_context_tool_argument_delta(text, previous_arguments, tier) {
        return runtime_smart_context_tool_argument_delta_summary(artifact, &delta);
    }
    runtime_smart_context_tool_argument_summary(artifact, text, tier)
}

pub(super) fn runtime_smart_context_tool_argument_summary(
    artifact: &runtime_proxy_crate::SmartContextArtifactRef,
    text: &str,
    tier: runtime_proxy_crate::SmartContextTokenBudgetTier,
) -> String {
    let reference = runtime_smart_context_artifact_ref(&artifact.id);
    let preview = text
        .chars()
        .take(runtime_smart_context_tool_args_preview_max_chars(tier))
        .collect::<String>();
    format!(
        "psc args {reference} b={} p:{}",
        artifact.byte_len,
        preview.trim()
    )
}

pub(super) fn runtime_smart_context_tool_argument_repeat_summary(
    artifact: &runtime_proxy_crate::SmartContextArtifactRef,
    previous_artifact: Option<&runtime_proxy_crate::SmartContextArtifactRef>,
) -> String {
    let reference = runtime_smart_context_artifact_ref(&artifact.id);
    let base_reference = previous_artifact
        .filter(|previous| previous.id != artifact.id)
        .map(|previous| format!(" same={}", runtime_smart_context_artifact_ref(&previous.id)))
        .unwrap_or_default();
    format!(
        "psc args rep {reference} b={}{}",
        artifact.byte_len, base_reference
    )
}

pub(super) fn runtime_smart_context_tool_argument_delta_summary(
    artifact: &runtime_proxy_crate::SmartContextArtifactRef,
    delta: &RuntimeSmartContextToolArgumentDelta,
) -> String {
    let reference = runtime_smart_context_artifact_ref(&artifact.id);
    let base_reference = runtime_smart_context_artifact_ref(&delta.base_artifact.id);
    let preview = if delta.inserted_preview.is_empty() {
        String::new()
    } else {
        format!(" p:{}", delta.inserted_preview.trim())
    };
    format!(
        "psc args d {reference} b={} base={base_reference} pre={} suf={} -{} +{} ih={}{}",
        artifact.byte_len,
        delta.prefix_bytes,
        delta.suffix_bytes,
        delta.removed_bytes,
        delta.inserted_bytes,
        delta.inserted_hash,
        preview
    )
}

pub(super) fn runtime_smart_context_tool_argument_delta(
    text: &str,
    previous_arguments: &[RuntimeSmartContextToolArgumentCandidate],
    tier: runtime_proxy_crate::SmartContextTokenBudgetTier,
) -> Option<RuntimeSmartContextToolArgumentDelta> {
    previous_arguments
        .iter()
        .filter_map(|previous| {
            runtime_smart_context_tool_argument_delta_against(text, previous, tier)
        })
        .max_by(|left, right| {
            left.common_bytes
                .cmp(&right.common_bytes)
                .then_with(|| right.changed_bytes.cmp(&left.changed_bytes))
                .then_with(|| left.base_artifact.id.cmp(&right.base_artifact.id))
        })
}

pub(super) fn runtime_smart_context_tool_argument_delta_against(
    text: &str,
    previous: &RuntimeSmartContextToolArgumentCandidate,
    tier: runtime_proxy_crate::SmartContextTokenBudgetTier,
) -> Option<RuntimeSmartContextToolArgumentDelta> {
    let base = previous.text.as_str();
    if text == base {
        return None;
    }

    let prefix_bytes = runtime_smart_context_common_prefix_boundary_len(text, base);
    let suffix_bytes = runtime_smart_context_common_suffix_boundary_len(text, base, prefix_bytes);
    let common_bytes = prefix_bytes.saturating_add(suffix_bytes);
    let max_len = text.len().max(base.len());
    if common_bytes < SMART_CONTEXT_TOOL_ARGS_DIFF_MIN_COMMON_BYTES
        || common_bytes.saturating_mul(100)
            < max_len.saturating_mul(SMART_CONTEXT_TOOL_ARGS_DIFF_MIN_COMMON_RATIO_PERCENT)
    {
        return None;
    }

    let removed_bytes = base
        .len()
        .saturating_sub(prefix_bytes)
        .saturating_sub(suffix_bytes);
    let inserted_bytes = text
        .len()
        .saturating_sub(prefix_bytes)
        .saturating_sub(suffix_bytes);
    let changed_bytes = removed_bytes.saturating_add(inserted_bytes);
    if changed_bytes.saturating_mul(100)
        > max_len.saturating_mul(SMART_CONTEXT_TOOL_ARGS_DIFF_MAX_CHANGED_RATIO_PERCENT)
    {
        return None;
    }

    let inserted_end = text.len().saturating_sub(suffix_bytes);
    let inserted = text.get(prefix_bytes..inserted_end)?;
    let inserted_preview = inserted
        .chars()
        .take(runtime_smart_context_tool_args_preview_max_chars(tier))
        .collect::<String>();
    Some(RuntimeSmartContextToolArgumentDelta {
        base_artifact: previous.artifact.clone(),
        prefix_bytes,
        suffix_bytes,
        removed_bytes,
        inserted_bytes,
        inserted_hash: runtime_proxy_crate::smart_context_hash_text(inserted),
        inserted_preview,
        common_bytes,
        changed_bytes,
    })
}
