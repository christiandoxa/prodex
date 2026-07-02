use crate::{ProviderEndpoint, ProviderId};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderCatalogFeatureFlags {
    pub tools: bool,
    pub json_schema: bool,
    pub vision: bool,
    pub audio: bool,
    pub web_search: bool,
    pub reasoning: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderCatalogEntry {
    pub provider: ProviderId,
    pub owned_by: String,
    pub id: String,
    pub display_name: String,
    pub description: String,
    pub context_window_tokens: Option<u64>,
    pub input_cost_per_million_microusd: Option<u64>,
    pub output_cost_per_million_microusd: Option<u64>,
    pub supported_endpoints: Vec<ProviderEndpoint>,
    pub aliases: Vec<String>,
    pub feature_flags: ProviderCatalogFeatureFlags,
    pub pricing_known: bool,
}

fn provider_catalog_entries_static() -> &'static [ProviderCatalogEntry] {
    static ENTRIES: OnceLock<Vec<ProviderCatalogEntry>> = OnceLock::new();
    ENTRIES
        .get_or_init(|| {
            serde_json::from_str(include_str!("../catalog/models.json"))
                .expect("provider catalog JSON should parse")
        })
        .as_slice()
}

pub fn provider_catalog_entries() -> &'static [ProviderCatalogEntry] {
    provider_catalog_entries_static()
}

pub fn provider_catalog_entries_for(provider: ProviderId) -> Vec<&'static ProviderCatalogEntry> {
    provider_catalog_entries_static()
        .iter()
        .filter(|entry| entry.provider == provider)
        .collect()
}

pub fn provider_catalog_entry(
    provider: ProviderId,
    model: &str,
) -> Option<&'static ProviderCatalogEntry> {
    let model = model.trim();
    provider_catalog_entries_static().iter().find(|entry| {
        entry.provider == provider
            && (entry.id.eq_ignore_ascii_case(model)
                || entry
                    .aliases
                    .iter()
                    .any(|alias| alias.eq_ignore_ascii_case(model)))
    })
}

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
            "input_cost_per_million_microusd": model.input_cost_per_million_microusd,
            "output_cost_per_million_microusd": model.output_cost_per_million_microusd,
            "endpoints": model.endpoints.iter().map(|endpoint| endpoint.label()).collect::<Vec<_>>(),
            "aliases": model.aliases,
        })
    })
}

pub fn provider_model_catalog_json(provider: ProviderId) -> Vec<serde_json::Value> {
    let catalog = provider_catalog_json(provider);
    if !catalog.is_empty() {
        return catalog;
    }
    crate::provider_model_catalog(provider)
        .iter()
        .map(|model| {
            serde_json::json!({
                "id": model.id,
                "object": "model",
                "provider": provider.label(),
                "owned_by": model.owned_by,
                "display_name": model.display_name,
                "description": model.description,
                "context_window": model.context_window_tokens,
                "input_cost_per_million_microusd": model.input_cost_per_million_microusd,
                "output_cost_per_million_microusd": model.output_cost_per_million_microusd,
                "endpoints": model.endpoints.iter().map(|endpoint| endpoint.label()).collect::<Vec<_>>(),
                "aliases": model.aliases,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_catalog_json_parses_and_covers_all_supported_providers() {
        let entries = provider_catalog_entries();
        assert!(!entries.is_empty());
        for provider in [
            ProviderId::OpenAi,
            ProviderId::Anthropic,
            ProviderId::Copilot,
            ProviderId::DeepSeek,
            ProviderId::Gemini,
            ProviderId::Local,
        ] {
            assert!(entries.iter().any(|entry| entry.provider == provider));
        }
    }

    #[test]
    fn provider_catalog_entries_have_unique_provider_scoped_ids_and_aliases() {
        let mut seen = std::collections::BTreeSet::new();
        for entry in provider_catalog_entries() {
            assert!(!entry.id.trim().is_empty());
            assert!(!entry.owned_by.trim().is_empty());
            assert!(!entry.supported_endpoints.is_empty());
            let key = format!(
                "{}:{}",
                entry.provider.label(),
                entry.id.to_ascii_lowercase()
            );
            assert!(seen.insert(key));
            for alias in &entry.aliases {
                assert!(!alias.trim().is_empty());
            }
        }
    }
}
