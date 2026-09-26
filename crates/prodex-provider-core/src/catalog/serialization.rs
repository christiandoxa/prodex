use super::{
    PROVIDER_MODEL_CATALOG_HARD_LIMIT, ProviderCatalogEntry, ProviderId,
    ProviderModelCatalogLimitError, ProviderModelChoice, provider_catalog_entries_for,
    provider_catalog_entry, resolve_provider_model_choices,
};

pub fn provider_catalog_json(provider: ProviderId) -> Vec<serde_json::Value> {
    provider_catalog_entries_for(provider)
        .into_iter()
        .map(provider_catalog_entry_json)
        .collect()
}

pub fn provider_catalog_entry_json(entry: &ProviderCatalogEntry) -> serde_json::Value {
    serde_json::json!({
        "id": entry.id,
        "object": "model",
        "provider": entry.provider.label(),
        "owned_by": entry.owned_by,
        "display_name": entry.display_name,
        "description": entry.description,
        "context_window": entry.context_window_tokens,
        "max_output_tokens": entry.max_output_tokens,
        "default_output_reserve_tokens": entry.default_output_reserve_tokens,
        "supported_reasoning_efforts": entry.supported_reasoning_efforts,
        "default_reasoning_effort": entry.default_reasoning_effort,
        "reasoning_reserve_tokens": entry.reasoning_reserve_tokens,
        "embedding_compatible": entry.embedding_compatible,
        "input_cost_per_million_microusd": entry.input_cost_per_million_microusd,
        "output_cost_per_million_microusd": entry.output_cost_per_million_microusd,
        "endpoints": entry.supported_endpoints.iter().map(|endpoint| endpoint.label()).collect::<Vec<_>>(),
        "aliases": entry.aliases,
        "feature_flags": entry.feature_flags,
        "pricing_known": entry.pricing_known,
    })
}

pub fn provider_model_json(provider: ProviderId, model: &str) -> Option<serde_json::Value> {
    if let Some(entry) = provider_catalog_entry(provider, model) {
        return Some(provider_catalog_entry_json(entry));
    }
    crate::provider_model_spec(provider, model).map(|model| {
        serde_json::json!({
            "id": model.id,
            "object": "model",
            "provider": provider.label(),
            "owned_by": model.owned_by,
            "display_name": model.display_name,
            "description": model.description,
            "context_window": model.context_window_tokens,
            "max_output_tokens": null,
            "default_output_reserve_tokens": null,
            "supported_reasoning_efforts": null,
            "default_reasoning_effort": null,
            "reasoning_reserve_tokens": null,
            "embedding_compatible": null,
            "input_cost_per_million_microusd": model.input_cost_per_million_microusd,
            "output_cost_per_million_microusd": model.output_cost_per_million_microusd,
            "endpoints": model.endpoints.iter().map(|endpoint| endpoint.label()).collect::<Vec<_>>(),
            "aliases": model.aliases,
        })
    })
}

pub fn provider_model_catalog_json(provider: ProviderId) -> Vec<serde_json::Value> {
    resolve_provider_model_choices(provider, &[], None)
        .into_iter()
        .filter_map(|choice| match choice {
            ProviderModelChoice::Model(model) => provider_model_json(provider, &model),
            ProviderModelChoice::ProviderDefault | ProviderModelChoice::Custom => None,
        })
        .collect()
}

/// Returns the canonical offline catalog followed by additional locally discovered models.
pub fn merge_provider_model_catalog_json<'a>(
    provider: ProviderId,
    additional_models: impl IntoIterator<Item = &'a serde_json::Value>,
) -> Result<Vec<serde_json::Value>, ProviderModelCatalogLimitError> {
    let mut models = provider_model_catalog_json(provider);
    if models.len() > PROVIDER_MODEL_CATALOG_HARD_LIMIT {
        return Err(ProviderModelCatalogLimitError { provider });
    }
    let additional_models = additional_models.into_iter().collect::<Vec<_>>();
    let additional = additional_models
        .iter()
        .enumerate()
        .filter_map(|(index, model)| {
            model
                .get("id")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|id| !id.is_empty())
                .map(|id| (index, id.to_string()))
        })
        .collect::<Vec<_>>();
    let additional_ids = additional
        .iter()
        .map(|(_, id)| id.as_str())
        .collect::<Vec<_>>();
    let accepted = super::merge_catalog_ids_with_mojo(provider, &additional_ids);
    for index in accepted {
        if models.len() >= PROVIDER_MODEL_CATALOG_HARD_LIMIT {
            return Err(ProviderModelCatalogLimitError { provider });
        }
        let original_index = additional
            .get(index)
            .map(|(original_index, _)| *original_index)
            .ok_or(ProviderModelCatalogLimitError { provider })?;
        models.push((*additional_models[original_index]).clone());
    }
    Ok(models)
}
