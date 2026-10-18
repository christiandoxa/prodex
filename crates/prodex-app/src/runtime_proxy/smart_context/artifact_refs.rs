#[cfg(test)]
use std::collections::BTreeMap;
use std::collections::BTreeSet;

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
    #[cfg(test)]
    aliases: BTreeMap<String, String>,
    references: Vec<RuntimeSmartContextArtifactReference>,
}

#[derive(serde::Deserialize)]
pub(super) struct RuntimeSmartContextRehydrationValue {
    pub(super) pointer: String,
    pub(super) references: Vec<RuntimeSmartContextArtifactReference>,
}

#[derive(serde::Deserialize)]
struct RehydrationProjection {
    values: Vec<RuntimeSmartContextRehydrationValue>,
}

fn runtime_smart_context_artifact_projection<T: serde::de::DeserializeOwned>(
    value: &serde_json::Value,
    operation: prodex_mojo_core::json::SmartContextArtifactProjection,
) -> T {
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
    runtime_smart_context_artifact_projection::<ArtifactProjection>(
        value,
        prodex_mojo_core::json::SmartContextArtifactProjection::Request,
    )
    .references
}

pub(super) fn runtime_smart_context_rehydration_values(
    value: &serde_json::Value,
) -> Vec<RuntimeSmartContextRehydrationValue> {
    runtime_smart_context_artifact_projection::<RehydrationProjection>(
        value,
        prodex_mojo_core::json::SmartContextArtifactProjection::RehydrationValues,
    )
    .values
}

#[cfg(test)]
#[path = "artifact_refs_tests.rs"]
mod tests;
