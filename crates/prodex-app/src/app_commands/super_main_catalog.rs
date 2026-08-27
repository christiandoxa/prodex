use super::super_prompt;
use crate::{AppPaths, canonical_sub_agent_efforts};
use prodex_cli::SubAgentReasoningEffort;
use std::collections::BTreeSet;
use std::fs;
use std::io::Read as IoRead;

const OPENAI_MODEL_CACHE_MAX_BYTES: u64 = 1024 * 1024;
const CODEX_MODEL_CACHE_MIN_VERSION: (u64, u64, u64) = (0, 150, 1);

#[derive(Clone, Debug)]
pub(super) struct MainModelChoice {
    pub(super) choice: prodex_provider_core::ProviderModelChoice,
    pub(super) label: String,
    pub(super) efforts: Option<Vec<String>>,
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
            },
        );
    }
    choices
}

fn main_model_choice_from_provider(
    provider: prodex_provider_core::ProviderId,
    choice: prodex_provider_core::ProviderModelChoice,
) -> MainModelChoice {
    let (label, efforts) = match &choice {
        prodex_provider_core::ProviderModelChoice::ProviderDefault => {
            ("provider default".to_string(), None)
        }
        prodex_provider_core::ProviderModelChoice::Model(model) => {
            let entry = prodex_provider_core::provider_catalog_entry(provider, model);
            (
                entry
                    .map(|entry| entry.display_name.clone())
                    .unwrap_or_else(|| model.clone()),
                entry
                    .and_then(|entry| {
                        entry.supported_reasoning_efforts.as_ref().map(|efforts| {
                            efforts
                                .iter()
                                .filter_map(|effort| sub_agent_effort(*effort))
                                .map(|effort| effort.as_str().to_string())
                                .collect()
                        })
                    })
                    .or_else(|| {
                        (provider == prodex_provider_core::ProviderId::OpenAi).then(|| {
                            canonical_sub_agent_efforts(provider, Some(model))
                                .into_iter()
                                .map(|effort| effort.as_str().to_string())
                                .collect()
                        })
                    }),
            )
        }
        prodex_provider_core::ProviderModelChoice::Custom => ("custom model...".to_string(), None),
    };
    MainModelChoice {
        choice,
        label,
        efforts,
    }
}

pub(super) fn main_model_choice_is_selectable(choices: &[MainModelChoice], model: &str) -> bool {
    choices.iter().any(|choice| {
        matches!(&choice.choice, prodex_provider_core::ProviderModelChoice::Model(candidate) if candidate.eq_ignore_ascii_case(model))
    })
}

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

pub(super) fn main_model_choices_from_catalog(
    mut entries: Vec<serde_json::Value>,
) -> Option<Vec<MainModelChoice>> {
    entries.sort_by(|left, right| {
        let left_priority = left
            .get("priority")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(u64::MAX);
        let right_priority = right
            .get("priority")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(u64::MAX);
        left_priority
            .cmp(&right_priority)
            .then_with(|| catalog_entry_model_id(left).cmp(catalog_entry_model_id(right)))
    });

    let mut seen = BTreeSet::new();
    let mut choices = vec![MainModelChoice {
        choice: prodex_provider_core::ProviderModelChoice::ProviderDefault,
        label: "provider default".to_string(),
        efforts: None,
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
        if model.is_empty() {
            continue;
        }
        if !seen.insert(model.to_ascii_lowercase()) {
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
        });
    }
    if choices.len() == 1 {
        return None;
    }
    choices.push(MainModelChoice {
        choice: prodex_provider_core::ProviderModelChoice::Custom,
        label: "custom model...".to_string(),
        efforts: None,
    });
    Some(choices)
}

fn catalog_entry_model_id(entry: &serde_json::Value) -> &str {
    ["slug", "id"]
        .into_iter()
        .find_map(|key| entry.get(key).and_then(serde_json::Value::as_str))
        .map(str::trim)
        .filter(|model| !model.is_empty())
        .unwrap_or("")
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
