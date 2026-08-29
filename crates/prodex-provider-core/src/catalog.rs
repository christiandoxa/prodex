use crate::{ProviderEndpoint, ProviderId, ProviderReasoningEffort};
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::BTreeMap;
#[cfg(any(not(feature = "mojo"), test))]
use std::collections::BTreeSet;
use std::fmt;
use std::sync::OnceLock;

pub const PROVIDER_MODEL_CATALOG_HARD_LIMIT: usize = 1_024;

fn deserialize_positive_context_window_tokens<'de, D>(
    deserializer: D,
) -> Result<Option<u64>, D::Error>
where
    D: Deserializer<'de>,
{
    match Option::<u64>::deserialize(deserializer)? {
        Some(tokens) if tokens > 0 => Ok(Some(tokens)),
        Some(_) => Err(serde::de::Error::custom(
            "context_window_tokens must be positive",
        )),
        None => Ok(None),
    }
}

#[cfg(all(test, feature = "mojo"))]
#[path = "catalog_parity_tests.rs"]
mod catalog_parity_tests;

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
    #[serde(deserialize_with = "deserialize_positive_context_window_tokens")]
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

#[path = "catalog/reasoning.rs"]
mod reasoning;
#[cfg(all(test, feature = "mojo"))]
pub(super) use reasoning::provider_model_reasoning_resolution_rust;
pub use reasoning::{
    ProviderModelReasoningError, ProviderModelReasoningResolution,
    provider_model_reasoning_resolution,
};
#[cfg(all(test, feature = "mojo"))]
pub(super) use reasoning::{reasoning_catalog_data, reasoning_effort_label};

#[path = "catalog/serialization.rs"]
mod serialization;
#[cfg(all(test, feature = "mojo"))]
pub(super) use serialization::merge_catalog_ids_rust;
pub use serialization::{provider_catalog_json, provider_model_catalog_json, provider_model_json};

pub fn merge_provider_model_catalog_json<'a>(
    provider: ProviderId,
    additional_models: impl IntoIterator<Item = &'a serde_json::Value>,
) -> Result<Vec<serde_json::Value>, ProviderModelCatalogLimitError> {
    serialization::merge_provider_model_catalog_json(provider, additional_models)
}

#[cfg(feature = "mojo")]
fn merge_catalog_ids_with_mojo(provider: ProviderId, additional: &[&str]) -> Vec<usize> {
    let entries = provider_catalog_entries_for(provider);
    let aliases = entries
        .iter()
        .map(|entry| entry.aliases.iter().map(String::as_str).collect::<Vec<_>>())
        .collect::<Vec<_>>();
    let catalog = entries
        .iter()
        .zip(&aliases)
        .map(|(entry, aliases)| prodex_mojo_core::rich::CatalogModel {
            id: entry.id.as_str(),
            aliases,
        })
        .collect::<Vec<_>>();
    prodex_mojo_core::rich::merge_catalog_ids(&catalog, additional)
        .expect("Mojo catalog merge returned an invalid structured result")
}

/// Builds the offline model picker from the canonical catalog plus local configuration.
#[cfg(feature = "mojo")]
pub fn resolve_provider_model_choices(
    provider: ProviderId,
    configured_models: &[String],
    current_model: Option<&str>,
) -> Vec<ProviderModelChoice> {
    let entries = provider_catalog_entries_for(provider);
    let configured = configured_models
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let aliases = entries
        .iter()
        .map(|entry| entry.aliases.iter().map(String::as_str).collect::<Vec<_>>())
        .collect::<Vec<_>>();
    let plan = prodex_mojo_core::rich::plan_catalog_choices(
        &entries
            .iter()
            .zip(&aliases)
            .map(|(entry, aliases)| prodex_mojo_core::rich::CatalogModel {
                id: entry.id.as_str(),
                aliases,
            })
            .collect::<Vec<_>>(),
        &configured,
        current_model,
    )
    .expect("Mojo catalog choice planning returned an invalid structured result");
    plan.into_iter()
        .map(|choice| match choice {
            prodex_mojo_core::rich::CatalogChoice::ProviderDefault => {
                ProviderModelChoice::ProviderDefault
            }
            prodex_mojo_core::rich::CatalogChoice::Catalog(index) => {
                ProviderModelChoice::Model(entries[index].id.clone())
            }
            prodex_mojo_core::rich::CatalogChoice::Configured(index) => {
                ProviderModelChoice::Model(normalize_model(provider, &configured_models[index]))
            }
            prodex_mojo_core::rich::CatalogChoice::Current => ProviderModelChoice::Model(
                normalize_model(provider, current_model.expect("current catalog choice")),
            ),
            prodex_mojo_core::rich::CatalogChoice::Custom => ProviderModelChoice::Custom,
        })
        .collect()
}

#[cfg(not(feature = "mojo"))]
pub fn resolve_provider_model_choices(
    provider: ProviderId,
    configured_models: &[String],
    current_model: Option<&str>,
) -> Vec<ProviderModelChoice> {
    resolve_provider_model_choices_rust(provider, configured_models, current_model)
}

#[cfg(any(not(feature = "mojo"), test))]
fn resolve_provider_model_choices_rust(
    provider: ProviderId,
    configured_models: &[String],
    current_model: Option<&str>,
) -> Vec<ProviderModelChoice> {
    let mut choices = vec![ProviderModelChoice::ProviderDefault];
    let mut seen = BTreeSet::new();
    let normalize = |model: &str| {
        provider_catalog_entry_rust(provider, model)
            .map(|entry| entry.id.as_str())
            .or_else(|| {
                crate::models::provider_model_spec_rust(provider, model).map(|spec| spec.id)
            })
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

#[cfg(feature = "mojo")]
fn normalize_model(provider: ProviderId, model: &str) -> String {
    provider_catalog_entry(provider, model)
        .map(|entry| entry.id.as_str())
        .or_else(|| crate::provider_model_spec(provider, model).map(|spec| spec.id))
        .unwrap_or(model)
        .to_string()
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

#[cfg(feature = "mojo")]
pub fn provider_catalog_entry(
    provider: ProviderId,
    model: &str,
) -> Option<&'static ProviderCatalogEntry> {
    let entries = provider_catalog_entries_static()
        .iter()
        .filter(|entry| entry.provider == provider)
        .collect::<Vec<_>>();
    let aliases = entries
        .iter()
        .map(|entry| entry.aliases.iter().map(String::as_str).collect::<Vec<_>>())
        .collect::<Vec<_>>();
    let catalog = entries
        .iter()
        .zip(&aliases)
        .map(|(entry, aliases)| prodex_mojo_core::rich::CatalogModel {
            id: entry.id.as_str(),
            aliases,
        })
        .collect::<Vec<_>>();
    prodex_mojo_core::rich::resolve_catalog_model(&catalog, model)
        .expect("Mojo catalog lookup returned an invalid structured result")
        .and_then(|index| entries.get(index).copied())
}

#[cfg(not(feature = "mojo"))]
pub fn provider_catalog_entry(
    provider: ProviderId,
    model: &str,
) -> Option<&'static ProviderCatalogEntry> {
    provider_catalog_entry_rust(provider, model)
}

#[cfg(any(not(feature = "mojo"), test))]
fn provider_catalog_entry_rust(
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

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
    fn provider_catalog_rejects_non_positive_context_windows() {
        let mut value = serde_json::json!({
            "provider": "openai",
            "owned_by": "example",
            "id": "test-model",
            "display_name": "Test Model",
            "description": "Synthetic test model.",
            "context_window_tokens": 1,
            "input_cost_per_million_microusd": null,
            "output_cost_per_million_microusd": null,
            "supported_endpoints": ["responses"],
            "aliases": [],
            "feature_flags": {
                "tools": false,
                "json_schema": false,
                "vision": false,
                "audio": false,
                "web_search": false,
                "reasoning": false
            },
            "pricing_known": false
        });

        for tokens in [0, -1] {
            value["context_window_tokens"] = serde_json::json!(tokens);
            assert!(serde_json::from_value::<ProviderCatalogEntry>(value.clone()).is_err());
        }
    }

    #[test]
    fn openai_picker_uses_catalog_order_and_gpt_5_6_efforts() {
        let choices = resolve_provider_model_choices(
            ProviderId::OpenAi,
            &["profile-model".to_string(), "gpt-5.6-luna".to_string()],
            Some("current-model"),
        );
        assert_eq!(choices[0], ProviderModelChoice::ProviderDefault);
        assert_eq!(
            choices[1],
            ProviderModelChoice::Model("gpt-5.6-sol".to_string())
        );
        assert_eq!(
            choices[2],
            ProviderModelChoice::Model("gpt-5.6-terra".to_string())
        );
        assert_eq!(
            choices[3],
            ProviderModelChoice::Model("gpt-5.6-luna".to_string())
        );
        assert!(choices.contains(&ProviderModelChoice::Model("profile-model".to_string())));
        assert!(choices.contains(&ProviderModelChoice::Model("current-model".to_string())));
        assert_eq!(choices.last(), Some(&ProviderModelChoice::Custom));
        for model in ["gpt-5.6-sol", "gpt-5.6-terra", "gpt-5.6-luna"] {
            let entry = provider_catalog_entry(ProviderId::OpenAi, model).unwrap();
            assert!(
                entry
                    .supported_reasoning_efforts
                    .as_ref()
                    .is_some_and(|efforts| efforts.contains(&ProviderReasoningEffort::Ultra))
                    || model == "gpt-5.6-luna"
            );
        }
        let luna = provider_catalog_entry(ProviderId::OpenAi, "gpt-5.6-luna").unwrap();
        assert_eq!(luna.context_window_tokens, Some(872_000));
        assert!(
            luna.supported_reasoning_efforts
                .as_ref()
                .is_some_and(|efforts| efforts.contains(&ProviderReasoningEffort::Max))
        );
    }

    #[test]
    fn copilot_and_kiro_luna_expose_verified_efforts() {
        let copilot = provider_catalog_entry(ProviderId::Copilot, "gpt-5.6-luna").unwrap();
        assert!(
            copilot
                .supported_reasoning_efforts
                .as_ref()
                .is_some_and(|efforts| efforts.contains(&ProviderReasoningEffort::Max))
        );
        let kiro = provider_catalog_entry(ProviderId::Kiro, "gpt-5.6-luna").unwrap();
        assert_eq!(
            kiro.supported_reasoning_efforts.as_deref(),
            Some(
                [
                    ProviderReasoningEffort::None,
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

    #[test]
    fn model_reasoning_resolution_is_model_scoped_and_explicit() {
        let resolution =
            provider_model_reasoning_resolution(ProviderId::OpenAi, Some("LUNA"), Some("MAX"))
                .unwrap();
        assert!(resolution.model_index.is_some());
        assert_eq!(
            resolution.selected_reasoning_effort,
            Some(ProviderReasoningEffort::Max)
        );
        assert_eq!(
            resolution.default_reasoning_effort,
            Some(ProviderReasoningEffort::Medium)
        );
        assert!(
            resolution
                .supported_reasoning_efforts
                .contains(&ProviderReasoningEffort::None)
        );
        assert_eq!(
            provider_model_reasoning_resolution(ProviderId::OpenAi, Some("luna"), Some("ultra")),
            Err(ProviderModelReasoningError::UnsupportedEffort)
        );
    }

    #[test]
    fn model_reasoning_resolution_does_not_invent_unknown_openai_model() {
        let resolution =
            provider_model_reasoning_resolution(ProviderId::OpenAi, Some("account/model"), None)
                .unwrap();
        assert_eq!(resolution.model_index, None);
        assert!(resolution.supported_reasoning_efforts.is_empty());
        assert_eq!(resolution.selected_reasoning_effort, None);
    }

    #[cfg(feature = "mojo")]
    #[test]
    fn model_reasoning_resolution_matches_rust_oracle_for_catalog_cases() {
        for (provider, model, effort) in [
            (ProviderId::OpenAi, Some("luna"), Some("max")),
            (ProviderId::Copilot, Some("luna"), Some("max")),
            (ProviderId::Kiro, Some("luna"), Some("xhigh")),
            (ProviderId::Gemini, Some("auto"), None),
            (ProviderId::OpenAi, Some("unknown"), Some("ultra")),
        ] {
            let (entries, efforts, _) = reasoning_catalog_data(provider);
            let expected = provider_model_reasoning_resolution_rust(
                &entries,
                &efforts,
                model,
                crate::provider_runtime_metadata(provider).map(|metadata| metadata.default_model),
                effort,
            )
            .map(|plan| {
                (
                    plan.model_index,
                    plan.supported_efforts,
                    plan.default_effort,
                    plan.selected_effort,
                )
            });
            let actual = provider_model_reasoning_resolution(provider, model, effort).map(|plan| {
                (
                    plan.model_index,
                    plan.supported_reasoning_efforts
                        .iter()
                        .filter_map(|effort| reasoning_effort_label(*effort))
                        .map(str::to_string)
                        .collect::<Vec<_>>(),
                    plan.default_reasoning_effort
                        .and_then(reasoning_effort_label)
                        .map(str::to_string),
                    plan.selected_reasoning_effort
                        .and_then(reasoning_effort_label)
                        .map(str::to_string),
                )
            });
            assert_eq!(actual, expected, "{provider:?} {model:?} {effort:?}");
        }
    }

    #[cfg(feature = "mojo")]
    #[test]
    fn mojo_catalog_preserves_provider_scoped_identity_and_order() {
        let entries = provider_catalog_entries_for(ProviderId::OpenAi);
        let first = entries[0];
        let alias = first
            .aliases
            .first()
            .map(String::as_str)
            .unwrap_or(first.id.as_str());
        assert_eq!(
            provider_catalog_entry(ProviderId::OpenAi, alias),
            Some(first)
        );

        let choices = resolve_provider_model_choices(
            ProviderId::OpenAi,
            &["custom-a".to_string(), first.id.to_ascii_uppercase()],
            Some("custom-b"),
        );
        assert_eq!(choices.first(), Some(&ProviderModelChoice::ProviderDefault));
        assert_eq!(choices.last(), Some(&ProviderModelChoice::Custom));
        assert!(choices.contains(&ProviderModelChoice::Model(first.id.clone())));
    }
}
