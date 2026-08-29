use super::super_prompt;
use crate::{AppPaths, canonical_sub_agent_efforts};
use prodex_cli::SubAgentReasoningEffort;
#[cfg(feature = "mojo-core")]
use prodex_mojo_core::rich::{
    CatalogConfigurationInput, CatalogConfigurationPlan, CatalogPlanModel, CatalogPlanRole,
    plan_catalog_configuration, plan_dynamic_catalog,
};
use std::collections::BTreeSet;
use std::fs;
use std::io::Read as IoRead;

const OPENAI_MODEL_CACHE_MAX_BYTES: u64 = 1024 * 1024;
const CODEX_MODEL_CACHE_MIN_VERSION: (u64, u64, u64) = (0, 150, 1);
const CATALOG_MAX_PRIORITY: u64 = i64::MAX as u64;
const CATALOG_MAX_IDENTIFIER_BYTES: usize = 4_096;
const CATALOG_MAX_QUERY_BYTES: usize = 65_536;
const CATALOG_MAX_INPUT_MODELS: usize = 65_536;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct MainModelChoice {
    pub(super) choice: prodex_provider_core::ProviderModelChoice,
    pub(super) label: String,
    pub(super) efforts: Option<Vec<String>>,
    pub(super) aliases: Vec<String>,
    pub(super) default_effort: Option<String>,
}

pub(super) fn main_model_choices(
    provider: prodex_provider_core::ProviderId,
    current_model: Option<&str>,
) -> Vec<MainModelChoice> {
    let mut choices = if provider == prodex_provider_core::ProviderId::OpenAi {
        openai_main_model_choices().unwrap_or_else(|| {
            prodex_provider_core::resolve_provider_model_choices(provider, &[], current_model)
                .into_iter()
                .map(|choice| main_model_choice_from_provider(provider, choice))
                .collect()
        })
    } else {
        let configured_models = super_prompt::configured_sub_agent_models(provider);
        prodex_provider_core::resolve_provider_model_choices(
            provider,
            &configured_models,
            current_model,
        )
        .into_iter()
        .map(|choice| main_model_choice_from_provider(provider, choice))
        .collect()
    };

    if let Some(model) = current_model
        .map(str::trim)
        .filter(|model| !model.is_empty())
        .filter(|model| !main_model_choice_is_selectable(&choices, model))
    {
        let insert_at = choices
            .iter()
            .position(|choice| {
                matches!(
                    choice.choice,
                    prodex_provider_core::ProviderModelChoice::Custom
                )
            })
            .unwrap_or(choices.len());
        let efforts = (provider == prodex_provider_core::ProviderId::OpenAi).then(|| {
            canonical_sub_agent_efforts(provider, Some(model))
                .into_iter()
                .map(|effort| effort.as_str().to_string())
                .collect()
        });
        choices.insert(
            insert_at,
            MainModelChoice {
                choice: prodex_provider_core::ProviderModelChoice::Model(model.to_string()),
                label: model.to_string(),
                efforts,
                aliases: Vec::new(),
                default_effort: None,
            },
        );
    }
    choices
}

fn main_model_choice_from_provider(
    provider: prodex_provider_core::ProviderId,
    choice: prodex_provider_core::ProviderModelChoice,
) -> MainModelChoice {
    let (label, efforts, aliases, default_effort) = match &choice {
        prodex_provider_core::ProviderModelChoice::ProviderDefault => {
            ("provider default".to_string(), None, Vec::new(), None)
        }
        prodex_provider_core::ProviderModelChoice::Model(model) => {
            let reasoning = prodex_provider_core::provider_model_reasoning_resolution(
                provider,
                Some(model),
                None,
            )
            .expect("provider model reasoning resolution failed");
            let efforts = if reasoning.model_index.is_some() {
                reasoning
                    .supported_reasoning_efforts
                    .iter()
                    .filter_map(|effort| sub_agent_effort(*effort))
                    .map(|effort| effort.as_str().to_string())
                    .collect()
            } else {
                canonical_sub_agent_efforts(provider, Some(model))
                    .into_iter()
                    .map(|effort| effort.as_str().to_string())
                    .collect()
            };
            let entry = prodex_provider_core::provider_catalog_entry(provider, model);
            let aliases = entry
                .map(|entry| entry.aliases.clone())
                .unwrap_or_default();
            let default_effort = reasoning
                .selected_reasoning_effort
                .and_then(sub_agent_effort)
                .map(|effort| effort.as_str().to_string());
            (
                entry
                    .map(|entry| entry.display_name.clone())
                    .unwrap_or_else(|| model.clone()),
                Some(efforts),
                aliases,
                default_effort,
            )
        }
        prodex_provider_core::ProviderModelChoice::Custom => {
            ("custom model...".to_string(), None, Vec::new(), None)
        }
    };
    MainModelChoice {
        choice,
        label,
        efforts,
        aliases,
        default_effort,
    }
}

pub(super) fn main_model_choice_matches(choice: &MainModelChoice, model: &str) -> bool {
    let prodex_provider_core::ProviderModelChoice::Model(candidate) = &choice.choice else {
        return false;
    };
    candidate.eq_ignore_ascii_case(model)
        || choice
            .aliases
            .iter()
            .any(|alias| alias.eq_ignore_ascii_case(model))
}

pub(super) fn main_model_choice_is_selectable(choices: &[MainModelChoice], model: &str) -> bool {
    choices
        .iter()
        .any(|choice| main_model_choice_matches(choice, model))
}

pub(super) fn provider_model_choice_matches(
    provider: prodex_provider_core::ProviderId,
    choice: &prodex_provider_core::ProviderModelChoice,
    model: &str,
) -> bool {
    let prodex_provider_core::ProviderModelChoice::Model(candidate) = choice else {
        return false;
    };
    candidate.eq_ignore_ascii_case(model)
        || prodex_provider_core::provider_catalog_entry(provider, candidate).is_some_and(|entry| {
            entry
                .aliases
                .iter()
                .any(|alias| alias.eq_ignore_ascii_case(model))
        })
}

#[cfg(not(feature = "mojo-core"))]
pub(super) fn first_main_catalog_model(choices: &[MainModelChoice]) -> Option<String> {
    choices.iter().find_map(|choice| match &choice.choice {
        prodex_provider_core::ProviderModelChoice::Model(model) => Some(model.clone()),
        _ => None,
    })
}

fn sub_agent_effort(
    effort: prodex_provider_core::ProviderReasoningEffort,
) -> Option<SubAgentReasoningEffort> {
    match effort {
        prodex_provider_core::ProviderReasoningEffort::None => Some(SubAgentReasoningEffort::None),
        prodex_provider_core::ProviderReasoningEffort::Minimal => {
            Some(SubAgentReasoningEffort::Minimal)
        }
        prodex_provider_core::ProviderReasoningEffort::Low => Some(SubAgentReasoningEffort::Low),
        prodex_provider_core::ProviderReasoningEffort::Medium => {
            Some(SubAgentReasoningEffort::Medium)
        }
        prodex_provider_core::ProviderReasoningEffort::High => Some(SubAgentReasoningEffort::High),
        prodex_provider_core::ProviderReasoningEffort::XHigh => {
            Some(SubAgentReasoningEffort::XHigh)
        }
        prodex_provider_core::ProviderReasoningEffort::Max => Some(SubAgentReasoningEffort::Max),
        prodex_provider_core::ProviderReasoningEffort::Ultra => {
            Some(SubAgentReasoningEffort::Ultra)
        }
        prodex_provider_core::ProviderReasoningEffort::Unknown => None,
    }
}

pub(super) fn openai_main_model_choices() -> Option<Vec<MainModelChoice>> {
    let paths = AppPaths::discover().ok()?;
    let codex_home = prodex_core::default_codex_home(&paths).ok()?;
    let file = fs::File::open(codex_home.join("models_cache.json")).ok()?;
    let mut contents = String::new();
    let mut bounded = IoRead::take(file, OPENAI_MODEL_CACHE_MAX_BYTES + 1);
    if IoRead::read_to_string(&mut bounded, &mut contents).is_err()
        || contents.len() as u64 > OPENAI_MODEL_CACHE_MAX_BYTES
    {
        return None;
    }
    let value = serde_json::from_str::<serde_json::Value>(&contents).ok()?;
    // Dynamic cache input is optional. The caller keeps the bundled catalog when
    // this planner reports invalid input; ABI and output failures remain hard.
    let mut choices = main_model_choices_from_catalog(value.get("models")?.as_array()?.to_vec())?;
    if !model_cache_is_current(
        value
            .get("client_version")
            .and_then(serde_json::Value::as_str),
    ) {
        merge_bundled_openai_choices(&mut choices);
    }
    Some(choices)
}

fn model_cache_is_current(version: Option<&str>) -> bool {
    let Some(version) = version else {
        return false;
    };
    let mut parts = version.split('.');
    let Some(major) = parts.next().and_then(|part| part.parse::<u64>().ok()) else {
        return false;
    };
    let Some(minor) = parts.next().and_then(|part| part.parse::<u64>().ok()) else {
        return false;
    };
    let Some(patch) = parts.next().and_then(|part| part.parse::<u64>().ok()) else {
        return false;
    };
    (major, minor, patch) >= CODEX_MODEL_CACHE_MIN_VERSION
}

fn merge_bundled_openai_choices(choices: &mut Vec<MainModelChoice>) {
    let mut seen = choices
        .iter()
        .filter_map(|choice| match &choice.choice {
            prodex_provider_core::ProviderModelChoice::Model(model) => {
                Some(model.to_ascii_lowercase())
            }
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    let insert_at = choices
        .iter()
        .position(|choice| {
            matches!(
                &choice.choice,
                prodex_provider_core::ProviderModelChoice::Custom
            )
        })
        .unwrap_or(choices.len());
    let bundled = prodex_provider_core::resolve_provider_model_choices(
        prodex_provider_core::ProviderId::OpenAi,
        &[],
        None,
    )
    .into_iter()
    .filter_map(|choice| match choice {
        prodex_provider_core::ProviderModelChoice::Model(model)
            if seen.insert(model.to_ascii_lowercase()) =>
        {
            Some(main_model_choice_from_provider(
                prodex_provider_core::ProviderId::OpenAi,
                prodex_provider_core::ProviderModelChoice::Model(model),
            ))
        }
        _ => None,
    })
    .collect::<Vec<_>>();
    choices.splice(insert_at..insert_at, bundled);
}

#[derive(Debug)]
struct DynamicCatalogModel {
    id: String,
    label: String,
    priority: u64,
    supported: bool,
    hidden: bool,
    listed: bool,
    efforts: Vec<String>,
    aliases: Vec<String>,
    default_effort: Option<String>,
}

fn dynamic_catalog_model(entry: serde_json::Value) -> DynamicCatalogModel {
    let id = catalog_entry_model_id(&entry).to_string();
    let label = ["display_name", "displayName"]
        .into_iter()
        .find_map(|key| entry.get(key).and_then(serde_json::Value::as_str))
        .map(str::trim)
        .filter(|label| !label.is_empty())
        .unwrap_or(id.as_str())
        .to_string();
    let efforts = entry
        .get("supported_reasoning_levels")
        .and_then(serde_json::Value::as_array)
        .map(|levels| {
            levels
                .iter()
                .filter_map(|level| level.get("effort").and_then(serde_json::Value::as_str))
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    DynamicCatalogModel {
        id,
        label,
        priority: entry
            .get("priority")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(CATALOG_MAX_PRIORITY),
        supported: entry
            .get("supported_in_api")
            .and_then(serde_json::Value::as_bool)
            != Some(false),
        hidden: entry.get("hidden").and_then(serde_json::Value::as_bool) == Some(true),
        listed: entry
            .get("visibility")
            .and_then(serde_json::Value::as_str)
            .is_none_or(|visibility| visibility.eq_ignore_ascii_case("list")),
        efforts,
        aliases: catalog_entry_aliases(&entry),
        default_effort: catalog_entry_default_effort(&entry),
    }
}

fn catalog_entry_aliases(entry: &serde_json::Value) -> Vec<String> {
    entry
        .get("aliases")
        .and_then(serde_json::Value::as_array)
        .map(|aliases| {
            aliases
                .iter()
                .filter_map(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|alias| !alias.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn catalog_entry_default_effort(entry: &serde_json::Value) -> Option<String> {
    ["default_reasoning_level", "default_reasoning_effort"]
        .into_iter()
        .find_map(|key| entry.get(key).and_then(serde_json::Value::as_str))
        .map(str::trim)
        .filter(|effort| !effort.is_empty())
        .map(str::to_string)
}

fn dynamic_catalog_input_is_bounded(entries: &[serde_json::Value]) -> bool {
    if entries.len() > prodex_provider_core::PROVIDER_MODEL_CATALOG_HARD_LIMIT {
        return false;
    }
    let mut effort_count = 0_usize;
    let mut alias_count = 0_usize;
    for entry in entries {
        if !catalog_entry_is_bounded(entry) {
            return false;
        }
        effort_count = effort_count.saturating_add(
            entry
                .get("supported_reasoning_levels")
                .and_then(serde_json::Value::as_array)
                .map_or(0, Vec::len),
        );
        alias_count = alias_count.saturating_add(catalog_entry_aliases(entry).len());
        if effort_count > CATALOG_MAX_INPUT_MODELS || alias_count > CATALOG_MAX_INPUT_MODELS {
            return false;
        }
    }
    true
}

#[cfg(feature = "mojo-core")]
pub(super) fn main_model_choices_from_catalog(
    entries: Vec<serde_json::Value>,
) -> Option<Vec<MainModelChoice>> {
    if !dynamic_catalog_input_is_bounded(&entries)
        || entries.iter().any(|entry| {
            entry
                .get("priority")
                .and_then(serde_json::Value::as_u64)
                .is_some_and(|priority| priority > CATALOG_MAX_PRIORITY)
        })
    {
        return None;
    }
    let owned = entries.into_iter().map(dynamic_catalog_model).collect::<Vec<_>>();
    let effort_views = owned
        .iter()
        .map(|entry| entry.efforts.iter().map(String::as_str).collect::<Vec<_>>())
        .collect::<Vec<_>>();
    let alias_views = owned
        .iter()
        .map(|entry| entry.aliases.iter().map(String::as_str).collect::<Vec<_>>())
        .collect::<Vec<_>>();
    let models = owned
        .iter()
        .zip(&effort_views)
        .zip(&alias_views)
        .map(|((entry, efforts), aliases)| CatalogPlanModel {
            id: &entry.id,
            aliases,
            label: &entry.label,
            priority: entry.priority,
            supported: entry.supported,
            hidden: entry.hidden,
            listed: entry.listed,
            efforts,
            default_effort: entry.default_effort.as_deref(),
        })
        .collect::<Vec<_>>();
    let plan = match plan_dynamic_catalog(&models) {
        Ok(plan) => plan,
        Err(prodex_mojo_core::MojoError::InvalidInput) => return None,
        Err(error) => panic!("Mojo dynamic catalog planning failed: {error:?}"),
    };
    if plan.models.is_empty() {
        return None;
    }
    let mut choices = vec![MainModelChoice {
        choice: prodex_provider_core::ProviderModelChoice::ProviderDefault,
        label: "provider default".to_string(),
        efforts: None,
        aliases: Vec::new(),
        default_effort: None,
    }];
    for model in plan.models {
        let Some(source) = owned
            .iter()
            .find(|entry| entry.id.trim() == model.id)
            .or_else(|| {
                owned
                    .iter()
                    .find(|entry| entry.id.trim().eq_ignore_ascii_case(&model.id))
            })
        else {
            return None;
        };
        choices.push(MainModelChoice {
            choice: prodex_provider_core::ProviderModelChoice::Model(model.id),
            label: model.label,
            efforts: (!model.efforts.is_empty()).then_some(model.efforts.clone()),
            aliases: source.aliases.clone(),
            default_effort: valid_default_effort(source.default_effort.as_deref(), &model.efforts),
        });
    }
    choices.push(MainModelChoice {
        choice: prodex_provider_core::ProviderModelChoice::Custom,
        label: "custom model...".to_string(),
        efforts: None,
        aliases: Vec::new(),
        default_effort: None,
    });
    Some(choices)
}

#[cfg(any(not(feature = "mojo-core"), test))]
pub(super) fn main_model_choices_from_catalog_rust(
    mut entries: Vec<serde_json::Value>,
) -> Option<Vec<MainModelChoice>> {
    if !dynamic_catalog_input_is_bounded(&entries)
        || entries.iter().any(|entry| {
            entry
                .get("priority")
                .and_then(serde_json::Value::as_u64)
                .is_some_and(|priority| priority > CATALOG_MAX_PRIORITY)
        })
    {
        return None;
    }
    entries.sort_by(|left, right| {
        let left_priority = left
            .get("priority")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(CATALOG_MAX_PRIORITY);
        let right_priority = right
            .get("priority")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(CATALOG_MAX_PRIORITY);
        left_priority
            .cmp(&right_priority)
            .then_with(|| catalog_entry_model_id(left).cmp(catalog_entry_model_id(right)))
    });
    let mut seen = BTreeSet::new();
    let mut choices = vec![MainModelChoice {
        choice: prodex_provider_core::ProviderModelChoice::ProviderDefault,
        label: "provider default".to_string(),
        efforts: None,
        aliases: Vec::new(),
        default_effort: None,
    }];
    for entry in entries {
        if entry
            .get("supported_in_api")
            .and_then(serde_json::Value::as_bool)
            == Some(false)
            || entry.get("hidden").and_then(serde_json::Value::as_bool) == Some(true)
            || entry
                .get("visibility")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|visibility| !visibility.eq_ignore_ascii_case("list"))
        {
            continue;
        }
        let model = catalog_entry_model_id(&entry).to_string();
        if model.is_empty() || !seen.insert(model.to_ascii_lowercase()) {
            continue;
        }
        let efforts = entry
            .get("supported_reasoning_levels")
            .and_then(serde_json::Value::as_array)
            .map(|levels| {
                let mut seen = BTreeSet::new();
                levels
                    .iter()
                    .filter_map(|level| {
                        level
                            .get("effort")
                            .and_then(serde_json::Value::as_str)
                            .map(str::trim)
                            .filter(|effort| !effort.is_empty())
                            .filter(|effort| seen.insert(effort.to_ascii_lowercase()))
                            .map(str::to_string)
                    })
                    .collect::<Vec<_>>()
            })
            .filter(|efforts| !efforts.is_empty());
        let aliases = catalog_entry_aliases(&entry);
        let default_effort = valid_default_effort(
            catalog_entry_default_effort(&entry).as_deref(),
            efforts.as_deref().unwrap_or_default(),
        );
        let label = ["display_name", "displayName"]
            .into_iter()
            .find_map(|key| entry.get(key).and_then(serde_json::Value::as_str))
            .map(str::trim)
            .filter(|label| !label.is_empty())
            .unwrap_or(model.as_str())
            .to_string();
        choices.push(MainModelChoice {
            choice: prodex_provider_core::ProviderModelChoice::Model(model),
            label,
            efforts,
            aliases,
            default_effort,
        });
    }
    if choices.len() == 1 {
        return None;
    }
    choices.push(MainModelChoice {
        choice: prodex_provider_core::ProviderModelChoice::Custom,
        label: "custom model...".to_string(),
        efforts: None,
        aliases: Vec::new(),
        default_effort: None,
    });
    Some(choices)
}

#[cfg(not(feature = "mojo-core"))]
pub(super) fn main_model_choices_from_catalog(
    entries: Vec<serde_json::Value>,
) -> Option<Vec<MainModelChoice>> {
    main_model_choices_from_catalog_rust(entries)
}

fn catalog_entry_model_id(entry: &serde_json::Value) -> &str {
    ["slug", "id"]
        .into_iter()
        .find_map(|key| entry.get(key).and_then(serde_json::Value::as_str))
        .map(str::trim)
        .filter(|model| !model.is_empty())
        .unwrap_or("")
}

fn catalog_entry_is_bounded(entry: &serde_json::Value) -> bool {
    let model = catalog_entry_model_id(entry);
    let label = ["display_name", "displayName"]
        .into_iter()
        .find_map(|key| entry.get(key).and_then(serde_json::Value::as_str))
        .map(str::trim)
        .filter(|label| !label.is_empty())
        .unwrap_or(model);
    model.len() <= CATALOG_MAX_IDENTIFIER_BYTES
        && label.len() <= CATALOG_MAX_QUERY_BYTES
        && catalog_entry_aliases(entry)
            .iter()
            .all(|alias| alias.len() <= CATALOG_MAX_IDENTIFIER_BYTES)
        && catalog_entry_default_effort(entry)
            .as_deref()
            .is_none_or(|effort| effort.len() <= CATALOG_MAX_QUERY_BYTES)
        && !entry
            .get("supported_reasoning_levels")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|levels| {
                levels.iter().any(|level| {
                    level
                        .get("effort")
                        .and_then(serde_json::Value::as_str)
                        .is_some_and(|effort| effort.len() > CATALOG_MAX_QUERY_BYTES)
                })
            })
}

fn valid_default_effort(default_effort: Option<&str>, efforts: &[String]) -> Option<String> {
    default_effort
        .map(str::trim)
        .filter(|default| !default.is_empty())
        .filter(|default| efforts.iter().any(|effort| effort.eq_ignore_ascii_case(default)))
        .map(str::to_string)
}

pub(super) fn prompt_main_model(
    title: &str,
    provider: prodex_provider_core::ProviderId,
    current_model: Option<&str>,
) -> anyhow::Result<Option<String>> {
    let models = main_model_choices(provider, current_model);
    let choices = models
        .iter()
        .map(|choice| choice.label.clone())
        .collect::<Vec<_>>();
    let selected = current_model
        .and_then(|model| {
            models.iter().position(|choice| {
                matches!(&choice.choice, prodex_provider_core::ProviderModelChoice::Model(candidate) if candidate.eq_ignore_ascii_case(model))
            })
        })
        .unwrap_or(0);
    let selected = super_prompt::prompt_super_choice(title, &choices, selected, false)?;
    Ok(match &models[selected].choice {
        prodex_provider_core::ProviderModelChoice::ProviderDefault => None,
        prodex_provider_core::ProviderModelChoice::Model(model) => Some(model.clone()),
        prodex_provider_core::ProviderModelChoice::Custom => Some(super_prompt::prompt_super_text(
            "Custom model",
            current_model.unwrap_or_default(),
        )?),
    })
}

pub(super) fn main_model_efforts(
    provider: prodex_provider_core::ProviderId,
    model: Option<&str>,
) -> Vec<String> {
    if let Some(efforts) = main_model_choices(provider, model)
        .iter()
        .find_map(|choice| {
            let prodex_provider_core::ProviderModelChoice::Model(candidate) = &choice.choice else {
                return None;
            };
            model
                .is_some_and(|model| candidate.eq_ignore_ascii_case(model))
                .then_some(choice.efforts.as_deref().unwrap_or_default())
        })
    {
        return efforts.to_vec();
    }
    canonical_sub_agent_efforts(provider, model)
        .into_iter()
        .map(|effort| effort.as_str().to_string())
        .collect()
}
