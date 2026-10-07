use super::super_prompt;
use crate::{canonical_sub_agent_efforts, effective_provider_model_catalog, provider_display_name};
use prodex_cli::SubAgentReasoningEffort;

const CATALOG_MAX_PRIORITY: u64 = i64::MAX as u64;
use prodex_mojo_core::rich::{
    CatalogChoicesPlan, CatalogModel, CatalogPlanModel, ascii_casefold_equal_exact,
    merge_catalog_ids, plan_dynamic_catalog, resolve_catalog_model_exact,
};

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
        let configured_models = effective_provider_model_catalog(provider).model_ids();
        prodex_provider_core::resolve_provider_model_choices(
            provider,
            &configured_models,
            current_model,
        )
        .into_iter()
        .map(|choice| main_model_choice_from_provider(provider, choice))
        .collect()
    };
    if provider == prodex_provider_core::ProviderId::OpenAi {
        choices.retain(|choice| {
            !matches!(
                &choice.choice,
                prodex_provider_core::ProviderModelChoice::Model(model)
                    if openai_model_is_retired(model)
            )
        });
    }

    if let Some(model) = current_model
        .map(str::trim)
        .filter(|model| !model.is_empty())
        .filter(|model| {
            provider != prodex_provider_core::ProviderId::OpenAi || !openai_model_is_retired(model)
        })
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

fn openai_model_is_retired(model: &str) -> bool {
    matches!(
        model.trim().to_ascii_lowercase().as_str(),
        "spark" | "gpt-5.3-codex-spark" | "gpt-5.3-spark"
    )
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
            let aliases = entry.map(|entry| entry.aliases.clone()).unwrap_or_default();
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
    let aliases = choice
        .aliases
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    resolve_catalog_model_exact(
        &[CatalogModel {
            id: candidate,
            aliases: &aliases,
        }],
        model,
    )
    .expect("Mojo exact main-model catalog resolution failed")
    .is_some()
}

pub(super) fn main_model_choice_is_selectable(choices: &[MainModelChoice], model: &str) -> bool {
    choices
        .iter()
        .any(|choice| main_model_choice_matches(choice, model))
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
    let catalog = effective_provider_model_catalog(prodex_provider_core::ProviderId::OpenAi);
    // Dynamic cache input is optional. The caller keeps the bundled catalog when
    // this planner reports invalid input; ABI and output failures remain hard.
    let mut choices = main_model_choices_from_catalog(catalog.models)?;
    merge_bundled_openai_choices(&mut choices);
    choices.retain(|choice| {
        !matches!(
            &choice.choice,
            prodex_provider_core::ProviderModelChoice::Model(model)
                if openai_model_is_retired(model)
        )
    });
    Some(choices)
}

fn merge_bundled_openai_choices(choices: &mut Vec<MainModelChoice>) {
    let existing_ids = choices
        .iter()
        .filter_map(|choice| match &choice.choice {
            prodex_provider_core::ProviderModelChoice::Model(model) => Some(model.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();
    let existing = existing_ids
        .iter()
        .map(|id| CatalogModel { id, aliases: &[] })
        .collect::<Vec<_>>();
    let bundled_models = prodex_provider_core::resolve_provider_model_choices(
        prodex_provider_core::ProviderId::OpenAi,
        &[],
        None,
    )
    .into_iter()
    .filter_map(|choice| match choice {
        prodex_provider_core::ProviderModelChoice::Model(model) => Some(model),
        _ => None,
    })
    .collect::<Vec<_>>();
    let bundled_ids = bundled_models
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let accepted = merge_catalog_ids(&existing, &bundled_ids)
        .expect("Mojo bundled OpenAI catalog merge failed");
    let insert_at = choices
        .iter()
        .position(|choice| {
            matches!(
                &choice.choice,
                prodex_provider_core::ProviderModelChoice::ProviderDefault
            )
        })
        .map_or(0, |index| index + 1);
    let bundled = accepted
        .into_iter()
        .map(|index| {
            main_model_choice_from_provider(
                prodex_provider_core::ProviderId::OpenAi,
                prodex_provider_core::ProviderModelChoice::Model(bundled_models[index].clone()),
            )
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
            .is_none_or(|visibility| {
                ascii_casefold_equal_exact(visibility, "list")
                    .expect("Mojo catalog visibility comparison failed")
            }),
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

fn main_model_catalog_plan_with_mojo(
    entries: Vec<serde_json::Value>,
) -> Option<(Vec<DynamicCatalogModel>, CatalogChoicesPlan)> {
    let owned = entries
        .into_iter()
        .map(dynamic_catalog_model)
        .collect::<Vec<_>>();
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
    Some((owned, plan))
}

pub(super) fn main_model_choices_from_catalog(
    entries: Vec<serde_json::Value>,
) -> Option<Vec<MainModelChoice>> {
    let (owned, plan) = main_model_catalog_plan_with_mojo(entries)?;
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
        let source = owned.iter().find(|entry| entry.id.trim() == model.id)?;
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

fn catalog_entry_model_id(entry: &serde_json::Value) -> &str {
    ["slug", "id"]
        .into_iter()
        .find_map(|key| entry.get(key).and_then(serde_json::Value::as_str))
        .map(str::trim)
        .filter(|model| !model.is_empty())
        .unwrap_or("")
}

fn valid_default_effort(default_effort: Option<&str>, efforts: &[String]) -> Option<String> {
    default_effort
        .map(str::trim)
        .filter(|default| !default.is_empty())
        .filter(|default| {
            efforts.iter().any(|effort| {
                ascii_casefold_equal_exact(effort, default)
                    .expect("Mojo catalog effort comparison failed")
            })
        })
        .map(str::to_string)
}

fn main_model_prompt_title(
    title: &str,
    provider: prodex_provider_core::ProviderId,
    degraded: bool,
) -> String {
    if degraded {
        format!(
            "{title} ({} account catalog degraded; available models shown)",
            provider_display_name(provider)
        )
    } else {
        title.to_string()
    }
}

pub(super) fn prompt_main_model(
    title: &str,
    provider: prodex_provider_core::ProviderId,
    current_model: Option<&str>,
) -> anyhow::Result<Option<String>> {
    let degraded = effective_provider_model_catalog(provider).is_degraded();
    let models = main_model_choices(provider, current_model);
    let choices = models
        .iter()
        .map(|choice| choice.label.clone())
        .collect::<Vec<_>>();
    let selected = current_model
        .and_then(|model| {
            models.iter().position(|choice| {
                matches!(
                    &choice.choice,
                    prodex_provider_core::ProviderModelChoice::Model(candidate)
                        if ascii_casefold_equal_exact(candidate, model)
                            .expect("Mojo main-model selection comparison failed")
                )
            })
        })
        .unwrap_or(0);
    let title = main_model_prompt_title(title, provider, degraded);
    let selected = super_prompt::prompt_super_choice(&title, &choices, selected, false)?;
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
                .is_some_and(|model| {
                    ascii_casefold_equal_exact(candidate, model)
                        .expect("Mojo main-model effort lookup comparison failed")
                })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn main_model_choice_matching_uses_exact_mojo_catalog_identity() {
        let choice = MainModelChoice {
            choice: prodex_provider_core::ProviderModelChoice::Model("gpt-main".to_string()),
            label: "Main".to_string(),
            efforts: None,
            aliases: vec!["gpt-alias".to_string()],
            default_effort: None,
        };
        assert!(main_model_choice_matches(&choice, "GPT-MAIN"));
        assert!(main_model_choice_matches(&choice, "GPT-ALIAS"));
        assert!(!main_model_choice_matches(&choice, " gpt-main "));
        assert!(!main_model_choice_matches(&choice, "other"));
    }

    #[test]
    fn bundled_openai_merge_keeps_canonical_case_insensitive_dedup() {
        let mut choices = vec![
            MainModelChoice {
                choice: prodex_provider_core::ProviderModelChoice::ProviderDefault,
                label: "provider default".to_string(),
                efforts: None,
                aliases: Vec::new(),
                default_effort: None,
            },
            MainModelChoice {
                choice: prodex_provider_core::ProviderModelChoice::Model("GPT-5.6-SOL".to_string()),
                label: "existing".to_string(),
                efforts: None,
                aliases: vec!["gpt-5.6-terra".to_string()],
                default_effort: None,
            },
            MainModelChoice {
                choice: prodex_provider_core::ProviderModelChoice::Custom,
                label: "custom model...".to_string(),
                efforts: None,
                aliases: Vec::new(),
                default_effort: None,
            },
        ];
        merge_bundled_openai_choices(&mut choices);
        let models = choices
            .iter()
            .filter_map(|choice| match &choice.choice {
                prodex_provider_core::ProviderModelChoice::Model(model) => Some(model.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            models
                .iter()
                .filter(|model| model.eq_ignore_ascii_case("gpt-5.6-sol"))
                .count(),
            1
        );
        assert!(
            models
                .iter()
                .any(|model| model.eq_ignore_ascii_case("gpt-5.6-terra"))
        );
    }

    #[test]
    fn degraded_main_model_title_is_bounded_and_non_secret() {
        assert_eq!(
            main_model_prompt_title(
                "Main-agent model",
                prodex_provider_core::ProviderId::Kiro,
                true,
            ),
            "Main-agent model (Kiro account catalog degraded; available models shown)"
        );
        assert_eq!(
            main_model_prompt_title(
                "Main-agent model",
                prodex_provider_core::ProviderId::Kiro,
                false,
            ),
            "Main-agent model"
        );
    }
}
