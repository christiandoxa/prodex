use crate::{
    AppPaths, AppState, AppStateIoExt, COPILOT_RUNTIME_MODEL_CATALOG_FILE, KIRO_MODEL_CATALOG_FILE,
    ProfileProvider, parse_kiro_model_catalog_text, read_provider_model_catalog_text,
};
use prodex_cli::SubAgentReasoningEffort;
use prodex_provider_core::{
    PROVIDER_IMPLEMENTATION_ORDER, ProviderId, ProviderModelChoice,
    provider_implementation_registry, provider_model_reasoning_resolution,
    resolve_provider_model_choices,
};
use serde_json::Value;
use std::collections::BTreeSet;
use std::path::PathBuf;

pub(crate) const OPENAI_MODEL_CACHE_FILE: &str = "models_cache.json";
pub(crate) const SUPER_CONFIGURED_MODEL_PROFILE_LIMIT: usize = 128;
pub(crate) const SUPER_CONFIGURED_MODEL_LIMIT: usize =
    prodex_provider_core::PROVIDER_MODEL_CATALOG_HARD_LIMIT;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DynamicCatalogStatus {
    NoDynamicCatalog,
    Available,
    Degraded,
}

#[derive(Clone, Debug)]
pub(crate) struct EffectiveProviderModelCatalog {
    pub(crate) models: Vec<Value>,
    pub(crate) status: DynamicCatalogStatus,
}

impl EffectiveProviderModelCatalog {
    pub(crate) fn model_ids(&self) -> Vec<String> {
        self.models
            .iter()
            .filter_map(catalog_entry_model_id)
            .map(str::to_string)
            .collect()
    }

    pub(crate) fn is_degraded(&self) -> bool {
        self.status == DynamicCatalogStatus::Degraded
    }
}

#[derive(Clone, Debug)]
struct CatalogSource {
    path: PathBuf,
    required: bool,
}

pub(crate) fn canonical_sub_agent_providers() -> &'static [ProviderId] {
    PROVIDER_IMPLEMENTATION_ORDER
}

pub(crate) fn effective_provider_model_catalog(
    provider: ProviderId,
) -> EffectiveProviderModelCatalog {
    let Ok(paths) = AppPaths::discover() else {
        return degraded_catalog();
    };
    effective_provider_model_catalog_from_paths(&paths, provider)
}

pub(crate) fn effective_provider_model_catalog_from_paths(
    paths: &AppPaths,
    provider: ProviderId,
) -> EffectiveProviderModelCatalog {
    let sources = match catalog_sources(paths, provider) {
        Ok(sources) => sources,
        Err(_) => return degraded_catalog(),
    };
    let mut models = Vec::new();
    let mut seen = BTreeSet::new();
    let mut degraded = false;
    let mut usable_sources = 0;
    let model_limit = SUPER_CONFIGURED_MODEL_LIMIT
        .saturating_sub(prodex_provider_core::provider_model_catalog_json(provider).len());

    for source in &sources {
        if usable_sources >= SUPER_CONFIGURED_MODEL_PROFILE_LIMIT {
            degraded = true;
            break;
        }
        let entries = match load_catalog_source(source, provider) {
            CatalogSourceLoad::Missing => {
                degraded |= source.required;
                continue;
            }
            CatalogSourceLoad::Invalid => {
                degraded = true;
                continue;
            }
            CatalogSourceLoad::Entries(entries) => entries,
        };
        let usable = append_catalog_entries(
            provider,
            entries,
            model_limit,
            &mut seen,
            &mut models,
            &mut degraded,
        );
        if !usable {
            degraded = true;
        } else if source.required {
            usable_sources += 1;
        }
        if models.len() >= model_limit {
            break;
        }
    }

    let status = dynamic_catalog_status(&models, degraded);
    EffectiveProviderModelCatalog { models, status }
}

enum CatalogSourceLoad {
    Missing,
    Invalid,
    Entries(Vec<Value>),
}

fn load_catalog_source(source: &CatalogSource, provider: ProviderId) -> CatalogSourceLoad {
    let contents = match read_provider_model_catalog_text(&source.path) {
        Ok(Some(contents)) => contents,
        Ok(None) => return CatalogSourceLoad::Missing,
        Err(_) => return CatalogSourceLoad::Invalid,
    };
    match parse_catalog(provider, &contents) {
        Ok(entries) if entries.len() <= prodex_provider_core::PROVIDER_MODEL_CATALOG_HARD_LIMIT => {
            CatalogSourceLoad::Entries(entries)
        }
        Ok(_) | Err(_) => CatalogSourceLoad::Invalid,
    }
}

fn append_catalog_entries(
    provider: ProviderId,
    entries: Vec<Value>,
    model_limit: usize,
    seen: &mut BTreeSet<String>,
    models: &mut Vec<Value>,
    degraded: &mut bool,
) -> bool {
    let mut usable = false;
    for entry in entries {
        let Some(id) = catalog_entry_model_id(&entry).map(str::to_string) else {
            continue;
        };
        if !catalog_entry_is_selectable(&entry) {
            continue;
        }
        usable = true;
        if !seen.insert(id.to_ascii_lowercase()) {
            continue;
        }
        models.push(catalog_entry_with_id(entry, provider, &id));
        if models.len() >= model_limit {
            *degraded = true;
            break;
        }
    }
    usable
}

fn dynamic_catalog_status(models: &[Value], degraded: bool) -> DynamicCatalogStatus {
    if degraded {
        DynamicCatalogStatus::Degraded
    } else if models.is_empty() {
        DynamicCatalogStatus::NoDynamicCatalog
    } else {
        DynamicCatalogStatus::Available
    }
}

fn degraded_catalog() -> EffectiveProviderModelCatalog {
    EffectiveProviderModelCatalog {
        models: Vec::new(),
        status: DynamicCatalogStatus::Degraded,
    }
}

fn catalog_sources(paths: &AppPaths, provider: ProviderId) -> anyhow::Result<Vec<CatalogSource>> {
    let mut sources = Vec::new();
    if let Some(file) = optional_catalog_file(provider) {
        sources.push(CatalogSource {
            path: prodex_core::default_codex_home(paths)?.join(file),
            required: false,
        });
    }
    let Some(profile_file) = profile_catalog_file(provider) else {
        return Ok(sources);
    };
    let state = AppState::load(paths)?;
    for profile in state.profiles.values() {
        if profile_provider_id(&profile.provider) != Some(provider) {
            continue;
        }
        sources.push(CatalogSource {
            path: profile.codex_home.join(profile_file),
            required: true,
        });
    }
    Ok(sources)
}

fn optional_catalog_file(provider: ProviderId) -> Option<&'static str> {
    match provider {
        ProviderId::OpenAi => Some(OPENAI_MODEL_CACHE_FILE),
        ProviderId::DeepSeek => Some("prodex-deepseek-model-catalog.json"),
        ProviderId::Gemini => Some("prodex-gemini-model-catalog.json"),
        ProviderId::Local => Some("prodex-local-model-catalog.json"),
        ProviderId::Anthropic | ProviderId::Copilot | ProviderId::Kiro => None,
    }
}

fn profile_catalog_file(provider: ProviderId) -> Option<&'static str> {
    match provider {
        ProviderId::Copilot => Some(COPILOT_RUNTIME_MODEL_CATALOG_FILE),
        ProviderId::Gemini => Some("prodex-gemini-model-catalog.json"),
        ProviderId::Kiro => Some(KIRO_MODEL_CATALOG_FILE),
        ProviderId::OpenAi | ProviderId::Anthropic | ProviderId::DeepSeek | ProviderId::Local => {
            None
        }
    }
}

fn profile_provider_id(provider: &ProfileProvider) -> Option<ProviderId> {
    match provider {
        ProfileProvider::Openai => Some(ProviderId::OpenAi),
        ProfileProvider::Gemini { .. } => Some(ProviderId::Gemini),
        ProfileProvider::Anthropic { .. } => Some(ProviderId::Anthropic),
        ProfileProvider::Copilot { .. } => Some(ProviderId::Copilot),
        ProfileProvider::Kiro { .. } => Some(ProviderId::Kiro),
        ProfileProvider::Agy { .. } => None,
    }
}

fn parse_catalog(provider: ProviderId, contents: &str) -> anyhow::Result<Vec<Value>> {
    if provider == ProviderId::Kiro {
        return parse_kiro_model_catalog_text(contents);
    }
    let value = serde_json::from_str::<Value>(contents)?;
    let models = value
        .get("models")
        .and_then(Value::as_array)
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("provider model catalog is missing models array"))?;
    Ok(models)
}

fn catalog_entry_model_id(value: &Value) -> Option<&str> {
    ["id", "model_id", "modelId", "slug", "model"]
        .into_iter()
        .find_map(|key| {
            value
                .get(key)
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|id| !id.is_empty())
        })
}

fn catalog_entry_with_id(mut value: Value, provider: ProviderId, id: &str) -> Value {
    if let Some(object) = value.as_object_mut() {
        if !object
            .get("id")
            .and_then(Value::as_str)
            .is_some_and(|id| !id.trim().is_empty())
        {
            object.insert("id".to_string(), Value::String(id.to_string()));
        }
        object.insert(
            "provider".to_string(),
            Value::String(provider.label().to_string()),
        );
    }
    value
}

fn catalog_entry_is_selectable(value: &Value) -> bool {
    value.get("supported_in_api").and_then(Value::as_bool) != Some(false)
        && value.get("hidden").and_then(Value::as_bool) != Some(true)
        && value
            .get("visibility")
            .and_then(Value::as_str)
            .is_none_or(|visibility| visibility.eq_ignore_ascii_case("list"))
}

pub(crate) fn canonical_sub_agent_model_choices(
    provider: ProviderId,
    current_model: Option<&str>,
) -> Vec<ProviderModelChoice> {
    resolve_provider_model_choices(provider, &[], current_model)
}

pub(crate) fn canonical_sub_agent_efforts(
    provider: ProviderId,
    model: Option<&str>,
) -> Vec<SubAgentReasoningEffort> {
    let resolution = provider_model_reasoning_resolution(provider, model, None)
        .expect("provider model reasoning resolution failed");
    if resolution.model_index.is_none() {
        return SubAgentReasoningEffort::ALL.to_vec();
    }

    resolution
        .supported_reasoning_efforts
        .iter()
        .filter_map(|effort| match effort {
            prodex_provider_core::ProviderReasoningEffort::None => {
                Some(SubAgentReasoningEffort::None)
            }
            prodex_provider_core::ProviderReasoningEffort::Minimal => {
                Some(SubAgentReasoningEffort::Minimal)
            }
            prodex_provider_core::ProviderReasoningEffort::Low => {
                Some(SubAgentReasoningEffort::Low)
            }
            prodex_provider_core::ProviderReasoningEffort::Medium => {
                Some(SubAgentReasoningEffort::Medium)
            }
            prodex_provider_core::ProviderReasoningEffort::High => {
                Some(SubAgentReasoningEffort::High)
            }
            prodex_provider_core::ProviderReasoningEffort::XHigh => {
                Some(SubAgentReasoningEffort::XHigh)
            }
            prodex_provider_core::ProviderReasoningEffort::Max => {
                Some(SubAgentReasoningEffort::Max)
            }
            prodex_provider_core::ProviderReasoningEffort::Ultra => {
                Some(SubAgentReasoningEffort::Ultra)
            }
            prodex_provider_core::ProviderReasoningEffort::Unknown => None,
        })
        .collect()
}

pub(crate) fn provider_display_name(provider: ProviderId) -> &'static str {
    provider_implementation_registry()
        .get(provider)
        .map(|descriptor| descriptor.display_name())
        .unwrap_or(provider.label())
}
