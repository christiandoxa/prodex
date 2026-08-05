use crate::{ProviderEndpoint, ProviderId, ProviderReasoningEffort};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fmt;
use std::sync::OnceLock;

pub const PROVIDER_MODEL_CATALOG_HARD_LIMIT: usize = 1_024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProviderModelCatalogLimitError {
    provider: ProviderId,
}

impl fmt::Display for ProviderModelCatalogLimitError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} model catalog exceeds the hard limit of {} entries",
            self.provider.label(),
            PROVIDER_MODEL_CATALOG_HARD_LIMIT
        )
    }
}

impl std::error::Error for ProviderModelCatalogLimitError {}

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
    #[serde(default)]
    pub max_output_tokens: Option<u64>,
    #[serde(default)]
    pub default_output_reserve_tokens: Option<u64>,
    #[serde(default)]
    pub supported_reasoning_efforts: Option<Vec<ProviderReasoningEffort>>,
    #[serde(default)]
    pub default_reasoning_effort: Option<ProviderReasoningEffort>,
    #[serde(default)]
    pub reasoning_reserve_tokens: Option<BTreeMap<ProviderReasoningEffort, u64>>,
    #[serde(default)]
    pub embedding_compatible: Option<bool>,
    pub input_cost_per_million_microusd: Option<u64>,
    pub output_cost_per_million_microusd: Option<u64>,
    pub supported_endpoints: Vec<ProviderEndpoint>,
    pub aliases: Vec<String>,
    pub feature_flags: ProviderCatalogFeatureFlags,
    pub pricing_known: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProviderModelChoice {
    ProviderDefault,
    Model(String),
    Custom,
}

/// Builds the offline model picker from the canonical catalog plus local configuration.
pub fn resolve_provider_model_choices(
    provider: ProviderId,
    configured_models: &[String],
    current_model: Option<&str>,
) -> Vec<ProviderModelChoice> {
    let mut choices = vec![ProviderModelChoice::ProviderDefault];
    let mut seen = BTreeSet::new();
    let normalize = |model: &str| {
        provider_catalog_entry(provider, model)
            .map(|entry| entry.id.as_str())
            .or_else(|| crate::provider_model_spec(provider, model).map(|spec| spec.id))
            .unwrap_or(model)
            .to_string()
    };
    for entry in provider_catalog_entries_for(provider) {
        let model = normalize(&entry.id);
        if seen.len() < PROVIDER_MODEL_CATALOG_HARD_LIMIT && seen.insert(model.to_ascii_lowercase())
        {
            choices.push(ProviderModelChoice::Model(model));
        }
    }
    let current = current_model
        .filter(|model| !model.trim().is_empty())
        .map(normalize);
    let current_key = current.as_ref().map(|model| model.to_ascii_lowercase());
    let reserve_current = usize::from(current_key.as_ref().is_some_and(|key| !seen.contains(key)));
    for model in configured_models {
        if model.trim().is_empty() {
            continue;
        }
        let model = normalize(model);
        let key = model.to_ascii_lowercase();
        if current_key.as_ref() == Some(&key) {
            continue;
        }
        if seen.len() >= PROVIDER_MODEL_CATALOG_HARD_LIMIT - reserve_current {
            break;
        }
        if seen.insert(key) {
            choices.push(ProviderModelChoice::Model(model));
        }
    }
    if let Some(model) = current
        && seen.len() < PROVIDER_MODEL_CATALOG_HARD_LIMIT
        && seen.insert(model.to_ascii_lowercase())
    {
        choices.push(ProviderModelChoice::Model(model));
    }
    choices.push(ProviderModelChoice::Custom);
    choices
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
    let mut seen = models
        .iter()
        .filter_map(|model| model.get("id").and_then(serde_json::Value::as_str))
        .map(|id| id.to_ascii_lowercase())
        .collect::<BTreeSet<_>>();
    for model in additional_models {
        let Some(id) = model
            .get("id")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|id| !id.is_empty())
        else {
            continue;
        };
        let canonical_id = provider_catalog_entry(provider, id)
            .map(|entry| entry.id.as_str())
            .or_else(|| crate::provider_model_spec(provider, id).map(|spec| spec.id))
            .unwrap_or(id);
        if seen.insert(canonical_id.to_ascii_lowercase()) {
            if models.len() >= PROVIDER_MODEL_CATALOG_HARD_LIMIT {
                return Err(ProviderModelCatalogLimitError { provider });
            }
            models.push(model.clone());
        }
    }
    Ok(models)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_catalog_json_parses_and_covers_all_supported_providers() {
        let entries = provider_catalog_entries();
        assert!(!entries.is_empty());
        for descriptor in crate::provider_implementation_registry().iter() {
            assert!(
                entries
                    .iter()
                    .any(|entry| entry.provider == descriptor.provider())
            );
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

    #[test]
    fn openai_picker_uses_catalog_order_and_luna_supports_max() {
        let choices = resolve_provider_model_choices(
            ProviderId::OpenAi,
            &["profile-model".to_string(), "gpt-5.6-luna".to_string()],
            Some("current-model"),
        );
        assert_eq!(choices[0], ProviderModelChoice::ProviderDefault);
        assert_eq!(
            choices[1],
            ProviderModelChoice::Model("gpt-5.6-luna".to_string())
        );
        assert!(choices.contains(&ProviderModelChoice::Model("profile-model".to_string())));
        assert!(choices.contains(&ProviderModelChoice::Model("current-model".to_string())));
        assert_eq!(choices.last(), Some(&ProviderModelChoice::Custom));
        let luna = provider_catalog_entry(ProviderId::OpenAi, "gpt-5.6-luna").unwrap();
        assert!(
            luna.supported_reasoning_efforts
                .as_ref()
                .is_some_and(|efforts| efforts.contains(&ProviderReasoningEffort::Max))
        );
    }

    #[test]
    fn copilot_luna_and_kiro_auto_expose_verified_efforts() {
        let copilot = provider_catalog_entry(ProviderId::Copilot, "gpt-5.6-luna").unwrap();
        assert!(
            copilot
                .supported_reasoning_efforts
                .as_ref()
                .is_some_and(|efforts| efforts.contains(&ProviderReasoningEffort::Max))
        );
        let kiro = provider_catalog_entry(ProviderId::Kiro, "auto").unwrap();
        assert_eq!(
            kiro.supported_reasoning_efforts.as_deref(),
            Some(
                [
                    ProviderReasoningEffort::Low,
                    ProviderReasoningEffort::Medium,
                    ProviderReasoningEffort::High,
                    ProviderReasoningEffort::XHigh,
                    ProviderReasoningEffort::Max,
                ]
                .as_slice()
            )
        );
    }

    #[test]
    fn every_picker_and_models_endpoint_include_the_full_static_catalog() {
        for descriptor in crate::provider_implementation_registry().iter() {
            let provider = descriptor.provider();
            let choices = resolve_provider_model_choices(provider, &[], None);
            let json = provider_model_catalog_json(provider);
            assert!(json.len() <= PROVIDER_MODEL_CATALOG_HARD_LIMIT);
            let detailed_ids = provider_catalog_entries_for(provider)
                .into_iter()
                .map(|entry| entry.id.as_str())
                .collect::<BTreeSet<_>>();
            let static_ids = descriptor
                .model_catalog()
                .iter()
                .map(|model| model.id)
                .collect::<BTreeSet<_>>();
            assert_eq!(
                detailed_ids,
                static_ids,
                "{} catalog drift",
                provider.label()
            );
            for entry in provider_catalog_entries_for(provider) {
                let static_model = descriptor
                    .model_catalog()
                    .iter()
                    .find(|model| model.id == entry.id)
                    .unwrap();
                assert_eq!(
                    entry.supported_endpoints,
                    static_model.endpoints,
                    "{}:{} endpoint drift",
                    provider.label(),
                    entry.id
                );
            }
            assert_eq!(choices.first(), Some(&ProviderModelChoice::ProviderDefault));
            assert_eq!(choices.last(), Some(&ProviderModelChoice::Custom));
            for model in descriptor.model_catalog() {
                assert!(
                    choices.contains(&ProviderModelChoice::Model(model.id.to_string())),
                    "{} picker omitted {}",
                    provider.label(),
                    model.id
                );
                assert!(
                    json.iter().any(|entry| entry["id"] == model.id),
                    "{} models endpoint omitted {}",
                    provider.label(),
                    model.id
                );
            }
        }
    }

    #[test]
    fn additional_model_catalog_entries_augment_canonical_models_stably() {
        let canonical = provider_model_catalog_json(ProviderId::OpenAi);
        let additional = [
            serde_json::json!({"id": "GPT-5.6-LUNA", "display_name": "duplicate"}),
            serde_json::json!({"id": "account/model:custom", "display_name": "Custom"}),
            serde_json::json!({"id": "ACCOUNT/MODEL:CUSTOM", "display_name": "duplicate"}),
            serde_json::json!({"id": "  "}),
        ];

        let merged = merge_provider_model_catalog_json(ProviderId::OpenAi, &additional).unwrap();

        assert_eq!(&merged[..canonical.len()], canonical.as_slice());
        assert_eq!(merged.len(), canonical.len() + 1);
        assert_eq!(merged.last().unwrap()["id"], "account/model:custom");
    }

    #[test]
    fn additional_model_catalog_entries_fail_explicitly_at_the_hard_limit() {
        let canonical_len = provider_model_catalog_json(ProviderId::OpenAi).len();
        let additional = (0..=PROVIDER_MODEL_CATALOG_HARD_LIMIT - canonical_len)
            .map(|index| serde_json::json!({"id": format!("custom-{index}")}))
            .collect::<Vec<_>>();

        let error = merge_provider_model_catalog_json(ProviderId::OpenAi, &additional).unwrap_err();

        assert_eq!(error.provider, ProviderId::OpenAi);
        assert!(error.to_string().contains("hard limit of 1024 entries"));
    }

    #[test]
    fn additional_model_catalog_accepts_exactly_the_hard_limit() {
        let canonical_len = provider_model_catalog_json(ProviderId::OpenAi).len();
        let additional = (0..PROVIDER_MODEL_CATALOG_HARD_LIMIT - canonical_len)
            .map(|index| serde_json::json!({"id": format!("custom-{index}")}))
            .collect::<Vec<_>>();

        let merged = merge_provider_model_catalog_json(ProviderId::OpenAi, &additional).unwrap();

        assert_eq!(merged.len(), PROVIDER_MODEL_CATALOG_HARD_LIMIT);
    }

    #[test]
    fn model_choices_keep_current_model_with_a_full_configured_catalog() {
        let configured = (0..PROVIDER_MODEL_CATALOG_HARD_LIMIT)
            .map(|index| format!("configured-{index}"))
            .collect::<Vec<_>>();

        let choices =
            resolve_provider_model_choices(ProviderId::OpenAi, &configured, Some("current-model"));

        assert_eq!(
            choices
                .iter()
                .filter(|choice| matches!(choice, ProviderModelChoice::Model(_)))
                .count(),
            PROVIDER_MODEL_CATALOG_HARD_LIMIT
        );
        assert!(choices.contains(&ProviderModelChoice::Model("current-model".to_string())));
        assert_eq!(choices.last(), Some(&ProviderModelChoice::Custom));
    }
}
