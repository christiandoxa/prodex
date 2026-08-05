use super::provider_bridge::RuntimeProviderBridgeKind;

pub(super) fn runtime_provider_model_catalog_json(
    kind: RuntimeProviderBridgeKind,
    dynamic_catalog: Option<&[serde_json::Value]>,
) -> Result<Vec<serde_json::Value>, prodex_provider_core::ProviderModelCatalogLimitError> {
    prodex_provider_core::merge_provider_model_catalog_json(
        kind.provider_id(),
        dynamic_catalog.unwrap_or_default(),
    )
}

pub(super) fn runtime_provider_model_json_for(
    kind: RuntimeProviderBridgeKind,
    model_catalog: &[serde_json::Value],
    model_id: &str,
) -> Option<serde_json::Value> {
    if let Some(model) = model_catalog.iter().find(|model| {
        model
            .get("id")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|id| id.eq_ignore_ascii_case(model_id))
    }) {
        return Some(model.clone());
    }
    prodex_provider_core::provider_model_json(kind.provider_id(), model_id)
}
