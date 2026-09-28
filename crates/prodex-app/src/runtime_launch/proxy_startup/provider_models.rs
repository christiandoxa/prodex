use super::provider_bridge::RuntimeProviderBridgeKind;
use prodex_mojo_core::rich::{CatalogModel, resolve_catalog_model_exact};

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
    let indexed = model_catalog
        .iter()
        .enumerate()
        .filter_map(|(index, model)| {
            model
                .get("id")
                .and_then(serde_json::Value::as_str)
                .map(|id| (index, id))
        })
        .collect::<Vec<_>>();
    let models = indexed
        .iter()
        .map(|(_, id)| CatalogModel { id, aliases: &[] })
        .collect::<Vec<_>>();
    if let Some(index) = resolve_catalog_model_exact(&models, model_id)
        .expect("Mojo exact runtime provider model resolution failed")
    {
        return Some(model_catalog[indexed[index].0].clone());
    }
    prodex_provider_core::provider_model_json(kind.provider_id(), model_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_provider_model_lookup_uses_exact_mojo_identity() {
        let catalog = vec![
            serde_json::json!({"id": "Dynamic-X", "source": "dynamic"}),
            serde_json::json!({"name": "ignored"}),
        ];
        assert_eq!(
            runtime_provider_model_json_for(
                RuntimeProviderBridgeKind::Gemini,
                &catalog,
                "dynamic-x",
            )
            .and_then(|model| model.get("source").cloned()),
            Some(serde_json::json!("dynamic"))
        );
        assert!(
            runtime_provider_model_json_for(
                RuntimeProviderBridgeKind::Gemini,
                &catalog,
                " dynamic-x ",
            )
            .is_none()
        );
    }
}
