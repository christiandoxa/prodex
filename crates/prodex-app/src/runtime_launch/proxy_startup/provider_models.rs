use super::provider_bridge::RuntimeProviderBridgeKind;

pub(super) fn runtime_provider_model_catalog_json(
    kind: RuntimeProviderBridgeKind,
) -> Vec<serde_json::Value> {
    prodex_provider_core::provider_model_catalog_json(kind.provider_id())
}

pub(super) fn runtime_provider_model_json_for(
    kind: RuntimeProviderBridgeKind,
    model_id: &str,
) -> Option<serde_json::Value> {
    prodex_provider_core::provider_model_json(kind.provider_id(), model_id)
}
