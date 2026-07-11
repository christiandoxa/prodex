use super::provider_bridge::RuntimeProviderBridgeKind;

pub(super) fn runtime_provider_model_catalog_json(
    kind: RuntimeProviderBridgeKind,
    dynamic_catalog: Option<&[serde_json::Value]>,
) -> Vec<serde_json::Value> {
    dynamic_catalog
        .filter(|catalog| !catalog.is_empty())
        .map(|catalog| catalog.to_vec())
        .unwrap_or_else(|| prodex_provider_core::provider_model_catalog_json(kind.provider_id()))
}

pub(super) fn runtime_provider_model_json_for(
    kind: RuntimeProviderBridgeKind,
    dynamic_catalog: Option<&[serde_json::Value]>,
    model_id: &str,
) -> Option<serde_json::Value> {
    if let Some(model) = dynamic_catalog.into_iter().flatten().find(|model| {
        model
            .get("id")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|id| id.eq_ignore_ascii_case(model_id))
    }) {
        return Some(model.clone());
    }
    prodex_provider_core::provider_model_json(kind.provider_id(), model_id)
}
