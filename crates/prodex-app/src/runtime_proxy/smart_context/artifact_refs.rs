use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Deserialize)]
pub(super) struct RuntimeSmartContextLineRange {
    pub(super) start: usize,
    pub(super) end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, serde::Deserialize)]
pub(super) struct RuntimeSmartContextArtifactReference {
    pub(super) id: String,
    pub(super) marker: String,
    pub(super) line_range: Option<RuntimeSmartContextLineRange>,
    pub(super) line_ranges: Vec<RuntimeSmartContextLineRange>,
}

#[derive(serde::Deserialize)]
struct ArtifactProjection {
    aliases: BTreeMap<String, String>,
    references: Vec<RuntimeSmartContextArtifactReference>,
}

fn runtime_smart_context_artifact_projection(
    value: &serde_json::Value,
    operation: prodex_mojo_core::json::SmartContextArtifactProjection,
) -> ArtifactProjection {
    let tree = crate::json_boundary::app_json_tree_without_wire(value)
        .expect("bounded Smart Context artifact JSON must fit its host ABI");
    let raw = std::str::from_utf8(&tree.raw).expect("Serde JSON remains UTF-8");
    let output =
        prodex_mojo_core::json::smart_context_artifact_projection(&tree.nodes, raw, operation)
            .expect("compiled Mojo artifact projection returned invalid output");
    serde_json::from_slice(&output).expect("Mojo artifact projection emits valid JSON")
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
    runtime_smart_context_artifact_projection(
        value,
        prodex_mojo_core::json::SmartContextArtifactProjection::Request,
    )
    .references
}

pub(super) fn runtime_smart_context_collect_artifact_aliases(
    value: &serde_json::Value,
) -> BTreeMap<String, String> {
    runtime_smart_context_artifact_projection(
        value,
        prodex_mojo_core::json::SmartContextArtifactProjection::Aliases,
    )
    .aliases
}

pub(super) fn runtime_smart_context_artifact_ref_occurrences_from_text(
    text: &str,
    aliases: &BTreeMap<String, String>,
) -> Vec<RuntimeSmartContextArtifactReference> {
    runtime_smart_context_artifact_projection(
        &serde_json::json!({"text": text, "aliases": aliases}),
        prodex_mojo_core::json::SmartContextArtifactProjection::Text,
    )
    .references
}

#[cfg(test)]
#[path = "artifact_refs_tests.rs"]
mod tests;
