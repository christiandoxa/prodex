use crate::{
    AppPaths, AppState, AppStateIoExt, COPILOT_RUNTIME_MODEL_CATALOG_FILE, KIRO_MODEL_CATALOG_FILE,
    ProfileProvider, parse_kiro_model_catalog_text, read_provider_model_catalog_text,
};
use prodex_cli::SubAgentReasoningEffort;
use prodex_mojo_core::rich::{CatalogModel, merge_catalog_ids};
use prodex_mojo_core::sub_agent_policy::{CatalogEntryPlan, CatalogStatusPlan};
use prodex_provider_core::{
    PROVIDER_IMPLEMENTATION_ORDER, ProviderId, ProviderModelChoice,
    provider_implementation_registry, provider_model_reasoning_resolution,
    resolve_provider_model_choices,
};
use serde_json::Value;
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
        let usable =
            append_catalog_entries(provider, entries, model_limit, &mut models, &mut degraded);
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
    models: &mut Vec<Value>,
    degraded: &mut bool,
) -> bool {
    let mut candidates = entries
        .into_iter()
        .filter_map(|entry| {
            let plan = catalog_entry_policy(&entry);
            if !plan.selectable {
                return None;
            }
            let Some((field, start, end)) = plan.model_id else {
                return None;
            };
            let fields = catalog_id_fields(&entry);
            let Some(id) = fields[field].and_then(|value| value.get(start..end)) else {
                return None;
            };
            let id = id.to_string();
            Some(Some((entry, id, plan.set_id)))
        })
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        return false;
    }

    let existing_ids = models
        .iter()
        .filter_map(catalog_entry_model_id)
        .collect::<Vec<_>>();
    let existing = existing_ids
        .iter()
        .map(|id| CatalogModel { id, aliases: &[] })
        .collect::<Vec<_>>();
    let candidate_ids = candidates
        .iter()
        .filter_map(|candidate| candidate.as_ref().map(|(_, id, _)| id.as_str()))
        .collect::<Vec<_>>();
    let accepted =
        merge_catalog_ids(&existing, &candidate_ids).expect("Mojo sub-agent catalog merge failed");

    for index in accepted {
        let (entry, id, set_id) = candidates
            .get_mut(index)
            .and_then(Option::take)
            .expect("Mojo catalog merge returned a valid candidate index");
        models.push(catalog_entry_with_id(entry, provider, &id, set_id));
        if models.len() >= model_limit {
            *degraded = true;
            break;
        }
    }
    true
}

fn dynamic_catalog_status(models: &[Value], degraded: bool) -> DynamicCatalogStatus {
    match prodex_mojo_core::sub_agent_policy::catalog_status(!models.is_empty(), degraded)
        .expect("Mojo sub-agent catalog status policy returned invalid output")
    {
        CatalogStatusPlan::NoDynamicCatalog => DynamicCatalogStatus::NoDynamicCatalog,
        CatalogStatusPlan::Available => DynamicCatalogStatus::Available,
        CatalogStatusPlan::Degraded => DynamicCatalogStatus::Degraded,
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
    let plan = catalog_entry_policy(value);
    let (field, start, end) = plan.model_id?;
    catalog_id_fields(value)[field].and_then(|id| id.get(start..end))
}

fn catalog_id_fields(value: &Value) -> [Option<&str>; 5] {
    ["id", "model_id", "modelId", "slug", "model"]
        .map(|field| value.get(field).and_then(Value::as_str))
}

fn catalog_entry_policy(value: &Value) -> CatalogEntryPlan {
    prodex_mojo_core::sub_agent_policy::catalog_entry_plan(
        catalog_id_fields(value),
        value.get("supported_in_api").and_then(Value::as_bool) == Some(false),
        value.get("hidden").and_then(Value::as_bool) == Some(true),
        value.get("visibility").and_then(Value::as_str),
    )
    .expect("Mojo sub-agent catalog entry policy returned invalid output")
}

fn catalog_entry_with_id(mut value: Value, provider: ProviderId, id: &str, set_id: bool) -> Value {
    if let Some(object) = value.as_object_mut() {
        if set_id {
            object.insert("id".to_string(), Value::String(id.to_string()));
        }
        object.insert(
            "provider".to_string(),
            Value::String(provider.label().to_string()),
        );
    }
    value
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
    if prodex_mojo_core::sub_agent_policy::use_all_effort_suggestions(
        resolution.model_index.is_some(),
    )
    .expect("Mojo sub-agent reasoning suggestion policy returned invalid output")
    {
        return SubAgentReasoningEffort::ALL.to_vec();
    }

    resolution
        .supported_reasoning_efforts
        .iter()
        .filter_map(|effort| effort.label()?.parse::<SubAgentReasoningEffort>().ok())
        .collect()
}

pub(crate) fn provider_display_name(provider: ProviderId) -> &'static str {
    provider_implementation_registry()
        .get(provider)
        .map(|descriptor| descriptor.display_name())
        .unwrap_or(provider.label())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_id_schema_and_visibility_rules_use_mojo_policy() {
        let entry = serde_json::json!({
            "id": "  ",
            "model_id": " model-from-id ",
            "slug": "lower-priority"
        });
        assert_eq!(catalog_entry_model_id(&entry), Some("model-from-id"));
        assert_eq!(
            catalog_entry_model_id(&serde_json::json!({"modelId": " camel "})),
            Some("camel")
        );
        assert_eq!(
            catalog_entry_model_id(&serde_json::json!({"id": " \t"})),
            None
        );

        assert!(catalog_entry_policy(&serde_json::json!({"visibility": "LIST"})).selectable);
        assert!(
            !catalog_entry_policy(&serde_json::json!({
                "visibility": "list "
            }))
            .selectable
        );
        assert!(
            !catalog_entry_policy(&serde_json::json!({
                "supported_in_api": false
            }))
            .selectable
        );
        assert!(!catalog_entry_policy(&serde_json::json!({"hidden": true})).selectable);
        assert!(catalog_entry_policy(&serde_json::json!({"visibility": 7})).selectable);
        assert_eq!(
            dynamic_catalog_status(&[], false),
            DynamicCatalogStatus::NoDynamicCatalog
        );
        assert_eq!(
            dynamic_catalog_status(&[], true),
            DynamicCatalogStatus::Degraded
        );
        assert_eq!(
            dynamic_catalog_status(&[serde_json::json!({"id": "model"})], false),
            DynamicCatalogStatus::Available
        );
    }
}
