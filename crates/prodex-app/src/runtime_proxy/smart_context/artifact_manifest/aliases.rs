use super::*;

#[cfg(test)]
pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_apply_artifact_aliases_to_generated_texts(
    value: &mut serde_json::Value,
) -> bool {
    let counts = runtime_smart_context_generated_artifact_ref_counts(value);
    let aliases = runtime_smart_context_artifact_alias_plan(counts, None);
    if aliases.is_empty() {
        return false;
    }
    let replacement_count =
        runtime_smart_context_replace_generated_artifact_refs_with_aliases(value, &aliases);
    if replacement_count == 0 {
        return false;
    }
    runtime_smart_context_insert_artifact_alias_legend(value, &aliases)
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_apply_artifact_aliases_to_generated_texts_with_state(
    value: &mut serde_json::Value,
    state: &mut RuntimeSmartContextProxyState,
) -> bool {
    let counts = runtime_smart_context_generated_artifact_ref_counts(value);
    let previous_aliases = state.artifact_aliases.clone();
    let previous_next_index = state.next_artifact_alias_index;
    let aliases = runtime_smart_context_artifact_alias_plan(counts, Some(state));
    if aliases.is_empty() {
        return false;
    }
    let replacement_count =
        runtime_smart_context_replace_generated_artifact_refs_with_aliases(value, &aliases);
    if replacement_count == 0 {
        state.artifact_aliases = previous_aliases;
        state.next_artifact_alias_index = previous_next_index;
        return false;
    }
    if !runtime_smart_context_insert_artifact_alias_legend(value, &aliases) {
        state.artifact_aliases = previous_aliases;
        state.next_artifact_alias_index = previous_next_index;
        return false;
    }
    true
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_generated_artifact_ref_counts(
    value: &serde_json::Value,
) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    runtime_smart_context_generated_artifact_ref_counts_from_value(value, &mut counts);
    counts
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_generated_artifact_ref_counts_from_value(
    value: &serde_json::Value,
    counts: &mut BTreeMap<String, usize>,
) {
    match value {
        serde_json::Value::String(text)
            if runtime_smart_context_text_is_generated_summary_or_manifest(text) =>
        {
            let aliases = BTreeMap::new();
            let ids = runtime_smart_context_artifact_ref_occurrences_from_text(text, &aliases)
                .into_iter()
                .map(|reference| reference.id)
                .collect::<BTreeSet<_>>();
            for id in ids {
                let reference = runtime_smart_context_artifact_ref(&id);
                let count = text.matches(&reference).count();
                if count > 0 {
                    *counts.entry(id).or_default() += count;
                }
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                runtime_smart_context_generated_artifact_ref_counts_from_value(item, counts);
            }
        }
        serde_json::Value::Object(object) => {
            for item in object.values() {
                runtime_smart_context_generated_artifact_ref_counts_from_value(item, counts);
            }
        }
        _ => {}
    }
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_artifact_alias_plan(
    counts: BTreeMap<String, usize>,
    mut state: Option<&mut RuntimeSmartContextProxyState>,
) -> Vec<RuntimeSmartContextArtifactAlias> {
    let mut candidates = counts
        .into_iter()
        .filter_map(|(id, count)| {
            (count > 1).then(|| {
                let reference = runtime_smart_context_artifact_ref(&id);
                let potential = reference.len().saturating_mul(count);
                (id, count, reference, potential)
            })
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| right.3.cmp(&left.3).then_with(|| left.0.cmp(&right.0)));

    let mut aliases = Vec::new();
    for (id, count, reference, _) in candidates {
        let alias = if let Some(state) = state.as_deref_mut() {
            runtime_smart_context_stable_artifact_alias_candidate(state, &id, &aliases)
        } else {
            format!("@{}", aliases.len())
        };
        if reference.len() <= alias.len() {
            continue;
        }
        let replacement_savings = reference
            .len()
            .saturating_sub(alias.len())
            .saturating_mul(count);
        let definition_len = alias
            .len()
            .saturating_add(1)
            .saturating_add(reference.len());
        let legend_cost = if aliases.is_empty() {
            SMART_CONTEXT_ARTIFACT_ALIAS_LEGEND_PREFIX
                .len()
                .saturating_add(definition_len)
                .saturating_add(1)
        } else {
            definition_len.saturating_add(1)
        };
        if replacement_savings <= legend_cost {
            continue;
        }
        if let Some(state) = state.as_deref_mut() {
            runtime_smart_context_commit_stable_artifact_alias(state, &id, &alias);
        }
        aliases.push(RuntimeSmartContextArtifactAlias { id, alias });
    }
    aliases
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_stable_artifact_alias_candidate(
    state: &mut RuntimeSmartContextProxyState,
    id: &str,
    planned_aliases: &[RuntimeSmartContextArtifactAlias],
) -> String {
    if let Some(alias) = state.artifact_aliases.get(id) {
        return alias.clone();
    }
    let mut index = state.next_artifact_alias_index;
    loop {
        let alias = format!("@{index}");
        index = index.saturating_add(1);
        if state
            .artifact_aliases
            .values()
            .all(|existing| existing != &alias)
            && planned_aliases.iter().all(|planned| planned.alias != alias)
        {
            return alias;
        }
    }
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_commit_stable_artifact_alias(
    state: &mut RuntimeSmartContextProxyState,
    id: &str,
    alias: &str,
) {
    if state
        .artifact_aliases
        .get(id)
        .is_some_and(|existing| existing == alias)
    {
        return;
    }
    state
        .artifact_aliases
        .insert(id.to_string(), alias.to_string());
    if let Some(index) = runtime_smart_context_artifact_alias_index(alias) {
        state.next_artifact_alias_index = state.next_artifact_alias_index.max(index + 1);
    }
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_artifact_alias_index(
    alias: &str,
) -> Option<usize> {
    alias.strip_prefix('@')?.parse::<usize>().ok()
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_persisted_artifact_aliases(
    state: &RuntimeSmartContextProxyState,
) -> Vec<RuntimeSmartContextPersistedArtifactAlias> {
    let mut aliases = state
        .artifact_aliases
        .iter()
        .filter(|(id, alias)| {
            runtime_smart_context_artifact_id_valid(id)
                && runtime_smart_context_artifact_alias_valid(alias)
        })
        .map(|(id, alias)| RuntimeSmartContextPersistedArtifactAlias {
            id: id.clone(),
            alias: alias.clone(),
        })
        .collect::<Vec<_>>();
    aliases.sort_by_key(|entry| {
        (
            runtime_smart_context_artifact_alias_index(&entry.alias).unwrap_or(usize::MAX),
            entry.id.clone(),
        )
    });
    aliases.truncate(SMART_CONTEXT_PERSISTED_ARTIFACT_ALIAS_LIMIT);
    aliases
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_artifact_alias_state_from_persisted(
    persisted: Vec<RuntimeSmartContextPersistedArtifactAlias>,
) -> (BTreeMap<String, String>, usize) {
    let aliases = runtime_smart_context_merge_persisted_artifact_aliases(Vec::new(), persisted);
    let mut map = BTreeMap::new();
    let mut next_index = 0usize;
    for entry in aliases {
        if let Some(index) = runtime_smart_context_artifact_alias_index(&entry.alias) {
            next_index = next_index.max(index + 1);
        }
        map.insert(entry.id, entry.alias);
    }
    (map, next_index)
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_merge_persisted_artifact_aliases(
    existing: Vec<RuntimeSmartContextPersistedArtifactAlias>,
    incoming: Vec<RuntimeSmartContextPersistedArtifactAlias>,
) -> Vec<RuntimeSmartContextPersistedArtifactAlias> {
    let mut by_id = BTreeMap::<String, String>::new();
    let mut alias_owner = BTreeMap::<String, String>::new();
    for entry in existing.into_iter().chain(incoming) {
        if !runtime_smart_context_artifact_id_valid(&entry.id)
            || !runtime_smart_context_artifact_alias_valid(&entry.alias)
        {
            continue;
        }
        if let Some(owner) = alias_owner.get(&entry.alias)
            && owner != &entry.id
        {
            continue;
        }
        if let Some(previous_alias) = by_id.insert(entry.id.clone(), entry.alias.clone()) {
            alias_owner.remove(&previous_alias);
        }
        alias_owner.insert(entry.alias, entry.id);
    }
    let mut aliases = by_id
        .into_iter()
        .map(|(id, alias)| RuntimeSmartContextPersistedArtifactAlias { id, alias })
        .collect::<Vec<_>>();
    aliases.sort_by_key(|entry| {
        (
            runtime_smart_context_artifact_alias_index(&entry.alias).unwrap_or(usize::MAX),
            entry.id.clone(),
        )
    });
    aliases.truncate(SMART_CONTEXT_PERSISTED_ARTIFACT_ALIAS_LIMIT);
    aliases
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_replace_generated_artifact_refs_with_aliases(
    value: &mut serde_json::Value,
    aliases: &[RuntimeSmartContextArtifactAlias],
) -> usize {
    match value {
        serde_json::Value::String(text)
            if runtime_smart_context_text_is_generated_summary_or_manifest(text) =>
        {
            let mut next_lines = Vec::new();
            let mut replacements = 0usize;
            for line in text.lines() {
                if runtime_smart_context_generated_control_line(line) {
                    next_lines.push(line.to_string());
                    continue;
                }
                let mut next_line = line.to_string();
                for alias in aliases {
                    let reference = runtime_smart_context_artifact_ref(&alias.id);
                    let count = next_line.matches(&reference).count();
                    if count == 0 {
                        continue;
                    }
                    next_line = next_line.replace(&reference, &alias.alias);
                    replacements = replacements.saturating_add(count);
                }
                next_lines.push(next_line);
            }
            if replacements > 0 {
                *text = next_lines.join("\n");
            }
            replacements
        }
        serde_json::Value::Array(items) => items
            .iter_mut()
            .map(|item| {
                runtime_smart_context_replace_generated_artifact_refs_with_aliases(item, aliases)
            })
            .sum(),
        serde_json::Value::Object(object) => object
            .values_mut()
            .map(|item| {
                runtime_smart_context_replace_generated_artifact_refs_with_aliases(item, aliases)
            })
            .sum(),
        _ => 0,
    }
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_generated_control_line(
    line: &str,
) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with(SMART_CONTEXT_ARTIFACT_ALIAS_LEGEND_PREFIX)
        || trimmed.starts_with(SMART_CONTEXT_ARTIFACT_ALIAS_LEGEND_PREFIX_LEGACY)
        || trimmed.starts_with(SMART_CONTEXT_PATH_ALIAS_LEGEND_PREFIX)
        || trimmed.starts_with(SMART_CONTEXT_PATH_ALIAS_LEGEND_PREFIX_LEGACY)
        || trimmed.starts_with("psc art ")
        || trimmed.starts_with("psc rep ")
        || trimmed.starts_with("psc repeat ")
        || trimmed.starts_with(SMART_CONTEXT_REPO_STATE_MARKER_PREFIX)
        || trimmed.starts_with(SMART_CONTEXT_REPO_STATE_MARKER_PREFIX_LEGACY)
        || trimmed.starts_with("prodex-sc artifact ")
        || trimmed.starts_with("prodex-sc repeat ")
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_insert_artifact_alias_legend(
    value: &mut serde_json::Value,
    aliases: &[RuntimeSmartContextArtifactAlias],
) -> bool {
    let legend = runtime_smart_context_artifact_alias_legend(aliases);
    runtime_smart_context_insert_artifact_alias_legend_in_value(value, &legend)
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_artifact_alias_legend(
    aliases: &[RuntimeSmartContextArtifactAlias],
) -> String {
    let defs = aliases
        .iter()
        .map(|alias| {
            format!(
                "{}={}",
                alias.alias,
                runtime_smart_context_artifact_ref(&alias.id)
            )
        })
        .collect::<Vec<_>>()
        .join(" ");
    format!("{SMART_CONTEXT_ARTIFACT_ALIAS_LEGEND_PREFIX}{defs}")
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_insert_artifact_alias_legend_in_value(
    value: &mut serde_json::Value,
    legend: &str,
) -> bool {
    match value {
        serde_json::Value::String(text)
            if runtime_smart_context_text_is_generated_summary_or_manifest(text) =>
        {
            *text = runtime_smart_context_insert_artifact_alias_legend_in_text(text, legend);
            true
        }
        serde_json::Value::Array(items) => items
            .iter_mut()
            .any(|item| runtime_smart_context_insert_artifact_alias_legend_in_value(item, legend)),
        serde_json::Value::Object(object) => object
            .values_mut()
            .any(|item| runtime_smart_context_insert_artifact_alias_legend_in_value(item, legend)),
        _ => false,
    }
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_insert_artifact_alias_legend_in_text(
    text: &str,
    legend: &str,
) -> String {
    let duplicate_legend = if legend.starts_with(SMART_CONTEXT_ARTIFACT_ALIAS_LEGEND_PREFIX) {
        text.contains(SMART_CONTEXT_ARTIFACT_ALIAS_LEGEND_PREFIX)
            || text.contains(SMART_CONTEXT_ARTIFACT_ALIAS_LEGEND_PREFIX_LEGACY)
    } else if legend.starts_with(SMART_CONTEXT_PATH_ALIAS_LEGEND_PREFIX) {
        text.contains(SMART_CONTEXT_PATH_ALIAS_LEGEND_PREFIX)
            || text.contains(SMART_CONTEXT_PATH_ALIAS_LEGEND_PREFIX_LEGACY)
    } else {
        text.contains(legend)
    };
    if duplicate_legend {
        return text.to_string();
    }
    if let Some((first, rest)) = text.split_once('\n') {
        format!("{first}\n{legend}\n{rest}")
    } else {
        format!("{text}\n{legend}")
    }
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_apply_path_aliases_to_generated_texts(
    value: &mut serde_json::Value,
) -> bool {
    match value {
        serde_json::Value::String(text)
            if runtime_smart_context_text_is_generated_summary_or_manifest(text) =>
        {
            let Some(next) = runtime_smart_context_path_aliased_generated_text(text) else {
                return false;
            };
            *text = next;
            true
        }
        serde_json::Value::Array(items) => items
            .iter_mut()
            .any(runtime_smart_context_apply_path_aliases_to_generated_texts),
        serde_json::Value::Object(object) => object
            .values_mut()
            .any(runtime_smart_context_apply_path_aliases_to_generated_texts),
        _ => false,
    }
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_path_aliased_generated_text(
    text: &str,
) -> Option<String> {
    let aliases = runtime_smart_context_path_aliases(text);
    if aliases.is_empty() {
        return None;
    }
    let mut candidate = text.to_string();
    for (alias, prefix) in &aliases {
        candidate = candidate.replace(prefix, alias);
    }
    if candidate == text || candidate.len() >= text.len() {
        return None;
    }
    let legend = aliases
        .iter()
        .map(|(alias, prefix)| format!("{alias}={prefix}"))
        .collect::<Vec<_>>()
        .join(" ");
    candidate = runtime_smart_context_insert_artifact_alias_legend_in_text(
        &candidate,
        &format!("{SMART_CONTEXT_PATH_ALIAS_LEGEND_PREFIX}{legend}"),
    );
    if candidate.len() < text.len()
        && prodex_context::critical_signal_self_check(text, &candidate).passed()
    {
        Some(candidate)
    } else {
        None
    }
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_path_aliases(
    text: &str,
) -> Vec<(String, String)> {
    let mut counts = BTreeMap::<String, usize>::new();
    for token in text.split_whitespace() {
        let token = token.trim_matches(|ch: char| {
            ch.is_ascii_punctuation() && !matches!(ch, '/' | '_' | '-' | '.')
        });
        if let Some(prefix) = runtime_smart_context_repeated_path_prefix(token) {
            *counts.entry(prefix).or_default() += 1;
        }
    }
    let mut candidates = counts
        .into_iter()
        .filter(|(prefix, count)| *count >= 2 && prefix.len() > 8)
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        right
            .0
            .len()
            .saturating_mul(right.1)
            .cmp(&left.0.len().saturating_mul(left.1))
    });
    candidates
        .into_iter()
        .take(4)
        .enumerate()
        .filter_map(|(index, (prefix, count))| {
            let alias = if index == 0 {
                "$R".to_string()
            } else {
                format!("$P{index}")
            };
            let saved = prefix
                .len()
                .saturating_sub(alias.len())
                .saturating_mul(count);
            let cost = alias.len().saturating_add(prefix.len()).saturating_add(2);
            (saved > cost).then_some((alias, prefix))
        })
        .collect()
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_repeated_path_prefix(
    token: &str,
) -> Option<String> {
    if !token.starts_with('/') {
        return None;
    }
    for marker in [
        "/crates/",
        "/src/",
        "/tests/",
        "/test/",
        "/dist/",
        "/target/",
        "/apps/",
        "/packages/",
    ] {
        if let Some(index) = token.find(marker) {
            let prefix = &token[..index];
            if prefix.matches('/').count() >= 2 {
                return Some(prefix.to_string());
            }
        }
    }
    token
        .rsplit_once('/')
        .map(|(prefix, _)| prefix)
        .filter(|prefix| prefix.matches('/').count() >= 3)
        .map(str::to_string)
}
