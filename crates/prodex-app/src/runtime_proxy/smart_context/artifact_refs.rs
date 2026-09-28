use super::constants::SMART_CONTEXT_SHORT_ARTIFACT_REF_PREFIX;
use super::static_context::{
    runtime_smart_context_static_prompt_field_key,
    runtime_smart_context_value_is_static_context_item,
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct RuntimeSmartContextLineRange {
    pub(super) start: usize,
    pub(super) end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct RuntimeSmartContextArtifactReference {
    pub(super) id: String,
    pub(super) marker: String,
    pub(super) line_range: Option<RuntimeSmartContextLineRange>,
    pub(super) line_ranges: Vec<RuntimeSmartContextLineRange>,
}

pub(super) fn runtime_smart_context_collect_rehydratable_artifact_ref_ids(
    value: &serde_json::Value,
) -> Vec<String> {
    runtime_smart_context_collect_rehydratable_artifact_refs(value)
        .into_iter()
        .map(|reference| reference.id)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

pub(super) fn runtime_smart_context_collect_rehydratable_artifact_refs(
    value: &serde_json::Value,
) -> Vec<RuntimeSmartContextArtifactReference> {
    let aliases = runtime_smart_context_collect_artifact_aliases(value);
    let mut refs = Vec::<RuntimeSmartContextArtifactReference>::new();
    runtime_smart_context_collect_rehydratable_artifact_refs_from_value(value, &aliases, &mut refs);
    refs
}

fn runtime_smart_context_collect_rehydratable_artifact_refs_from_value(
    value: &serde_json::Value,
    aliases: &BTreeMap<String, String>,
    refs: &mut Vec<RuntimeSmartContextArtifactReference>,
) {
    if runtime_smart_context_value_is_static_context_item(value) {
        return;
    }
    if let Some(object) = value.as_object() {
        for (key, item) in object {
            if runtime_smart_context_static_prompt_field_key(key) {
                continue;
            }
            runtime_smart_context_collect_rehydratable_artifact_refs_from_value(
                item, aliases, refs,
            );
        }
        return;
    }
    if let Some(items) = value.as_array() {
        for item in items {
            runtime_smart_context_collect_rehydratable_artifact_refs_from_value(
                item, aliases, refs,
            );
        }
        return;
    }
    runtime_smart_context_collect_artifact_refs_from_value(value, aliases, refs);
}

fn runtime_smart_context_collect_artifact_refs_from_value<
    R: Extend<RuntimeSmartContextArtifactReference>,
>(
    value: &serde_json::Value,
    aliases: &BTreeMap<String, String>,
    refs: &mut R,
) {
    if let Some(text) = value.as_str() {
        if runtime_smart_context_may_contain_artifact_ref(text) {
            for reference in runtime_smart_context_artifact_ref_occurrences_from_text(text, aliases)
            {
                refs.extend([reference]);
            }
        }
        return;
    }
    if let Some(items) = value.as_array() {
        for item in items {
            runtime_smart_context_collect_artifact_refs_from_value(item, aliases, refs);
        }
        return;
    }
    if let Some(object) = value.as_object() {
        for item in object.values() {
            runtime_smart_context_collect_artifact_refs_from_value(item, aliases, refs);
        }
    }
}

fn runtime_smart_context_may_contain_artifact_ref(text: &str) -> bool {
    text.contains("prodex-artifact:")
        || text.contains(SMART_CONTEXT_SHORT_ARTIFACT_REF_PREFIX)
        || text.contains("psc2:")
        || text.contains('@')
        || text.contains("prodex smart context artifact")
        || text.contains("prodex-sc ")
}

pub(super) fn runtime_smart_context_collect_artifact_aliases(
    value: &serde_json::Value,
) -> BTreeMap<String, String> {
    let mut aliases = BTreeMap::new();
    runtime_smart_context_collect_artifact_aliases_from_value(value, &mut aliases);
    aliases
}

fn runtime_smart_context_collect_artifact_aliases_from_value(
    value: &serde_json::Value,
    aliases: &mut BTreeMap<String, String>,
) {
    if let Some(text) = value.as_str() {
        runtime_smart_context_collect_artifact_aliases_from_text(text, aliases);
        return;
    }
    if let Some(items) = value.as_array() {
        for item in items {
            runtime_smart_context_collect_artifact_aliases_from_value(item, aliases);
        }
        return;
    }
    if let Some(object) = value.as_object() {
        for item in object.values() {
            runtime_smart_context_collect_artifact_aliases_from_value(item, aliases);
        }
    }
}

fn runtime_smart_context_collect_artifact_aliases_from_text(
    text: &str,
    aliases: &mut BTreeMap<String, String>,
) {
    if !text.contains('@') || !text.contains('=') {
        return;
    }
    for (alias, id) in runtime_smart_context_artifact_ref_tokens(text)
        .into_iter()
        .filter_map(runtime_smart_context_parse_artifact_alias)
    {
        aliases.entry(alias).or_insert(id);
    }
}

pub(super) fn runtime_smart_context_artifact_ref_occurrences_from_text(
    text: &str,
    aliases: &BTreeMap<String, String>,
) -> Vec<RuntimeSmartContextArtifactReference> {
    runtime_smart_context_artifact_ref_tokens(text)
        .into_iter()
        .filter_map(|token| {
            runtime_smart_context_parse_artifact_reference_with_aliases(token, aliases)
        })
        .collect()
}

fn runtime_smart_context_artifact_ref_tokens(text: &str) -> Vec<&str> {
    text.split(|ch: char| ch.is_whitespace() || matches!(ch, ')' | ']' | '}'))
        .collect()
}

fn runtime_smart_context_parse_artifact_alias(token: &str) -> Option<(String, String)> {
    let plan = prodex_mojo_core::smart_context_artifact_ref::parse_alias_declaration(token)
        .expect("Mojo smart-context artifact alias declaration returned invalid output")?;
    let alias = token.get(plan.alias_start..plan.alias_end)?.to_string();
    Some((alias, plan.id))
}

fn runtime_smart_context_parse_artifact_reference_with_aliases(
    token: &str,
    aliases: &BTreeMap<String, String>,
) -> Option<RuntimeSmartContextArtifactReference> {
    if prodex_mojo_core::smart_context_artifact_ref::parse_alias_declaration(token)
        .expect("Mojo smart-context artifact alias declaration returned invalid output")
        .is_some()
    {
        return None;
    }
    if let Some(reference) = runtime_smart_context_parse_alias_artifact_reference(token, aliases) {
        return Some(reference);
    }
    runtime_smart_context_parse_non_alias_artifact_reference(token)
}

fn runtime_smart_context_parse_alias_artifact_reference(
    token: &str,
    aliases: &BTreeMap<String, String>,
) -> Option<RuntimeSmartContextArtifactReference> {
    let plan = prodex_mojo_core::smart_context_artifact_ref::parse_alias_reference(token)
        .expect("Mojo smart-context artifact alias reference returned invalid output")?;
    let alias = token.get(plan.alias_start..plan.alias_end)?;
    let id = aliases.get(alias)?;
    let marker = token.get(plan.marker_start..plan.marker_end)?.to_string();
    let line_ranges = plan
        .line_ranges
        .into_iter()
        .map(|range| RuntimeSmartContextLineRange {
            start: range.start,
            end: range.end,
        })
        .collect::<Vec<_>>();
    Some(RuntimeSmartContextArtifactReference {
        id: id.clone(),
        marker,
        line_range: line_ranges.first().copied(),
        line_ranges,
    })
}

pub(super) fn runtime_smart_context_parse_non_alias_artifact_reference(
    token: &str,
) -> Option<RuntimeSmartContextArtifactReference> {
    let plan = prodex_mojo_core::smart_context_artifact_ref::parse_reference(token)
        .expect("Mojo smart-context artifact reference parser returned invalid output")?;
    let marker = token.get(plan.marker_start..plan.marker_end)?.to_string();
    let line_ranges = plan
        .line_ranges
        .into_iter()
        .map(|range| RuntimeSmartContextLineRange {
            start: range.start,
            end: range.end,
        })
        .collect::<Vec<_>>();
    Some(RuntimeSmartContextArtifactReference {
        id: plan.id,
        marker,
        line_range: line_ranges.first().copied(),
        line_ranges,
    })
}
