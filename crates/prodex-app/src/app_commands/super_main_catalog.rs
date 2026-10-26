use super::super_prompt;
use crate::{canonical_sub_agent_efforts, effective_provider_model_catalog, provider_display_name};
use prodex_cli::SubAgentReasoningEffort;

use prodex_mojo_core::rich::{
    CatalogChoicesPlan, CatalogInsertionChoice, CatalogInsertionPlacement, CatalogModel,
    CatalogPlanModel, ascii_casefold_equal_exact, catalog_insertion_index, plan_dynamic_catalog,
    resolve_catalog_model_exact,
};
use prodex_provider_core::{MainCatalogModel as DynamicCatalogModel, project_main_catalog_models};

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
    let catalog = effective_provider_model_catalog(provider);
    let discovered = catalog.model_ids();
    let allowed = catalog.choices(provider, current_model, &discovered);
    let fallback = || {
        allowed
            .iter()
            .cloned()
            .map(|choice| main_model_choice_from_provider(provider, choice))
            .collect()
    };
    let mut choices = if provider == prodex_provider_core::ProviderId::OpenAi {
        main_model_choices_from_catalog(catalog.models.clone()).unwrap_or_else(fallback)
    } else {
        fallback()
    };
    choices.retain(|choice| {
        !matches!(&choice.choice,
        prodex_provider_core::ProviderModelChoice::Model(model)
            if catalog.model_is_retired(provider, model))
    });

    if let Some(model) = current_model
        .map(str::trim)
        .filter(|model| !model.is_empty())
        .filter(|model| {
            let canonical = prodex_provider_core::provider_catalog_entry(provider, model)
                .map(|entry| entry.id.as_str())
                .unwrap_or(model);
            allowed.iter().any(|choice| {
                matches!(choice,
                prodex_provider_core::ProviderModelChoice::Model(id)
                    if ascii_casefold_equal_exact(id, canonical)
                        .expect("compiled current-model identity matching failed"))
            })
        })
        .filter(|model| !main_model_choice_is_selectable(&choices, model))
    {
        let insert_at = main_model_choice_insertion_index(
            &choices,
            CatalogInsertionPlacement::CurrentBeforeCustom,
        );
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

/// Convert host model-choice variants to compiled catalog shape tags.
/// Placement and first-match precedence are Mojo-owned.
fn main_model_choice_insertion_index(
    choices: &[MainModelChoice],
    placement: CatalogInsertionPlacement,
) -> usize {
    let kinds = choices
        .iter()
        .map(|choice| match choice.choice {
            prodex_provider_core::ProviderModelChoice::ProviderDefault => {
                CatalogInsertionChoice::ProviderDefault
            }
            prodex_provider_core::ProviderModelChoice::Model(_) => CatalogInsertionChoice::Model,
            prodex_provider_core::ProviderModelChoice::Custom => CatalogInsertionChoice::Custom,
        })
        .collect::<Vec<_>>();
    catalog_insertion_index(&kinds, placement)
        .expect("compiled Mojo main model picker insertion index invalid")
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

#[cfg(test)]
pub(super) fn openai_main_model_choices() -> Option<Vec<MainModelChoice>> {
    let provider = prodex_provider_core::ProviderId::OpenAi;
    let catalog = effective_provider_model_catalog(provider);
    main_model_choices_from_catalog(catalog.models)
}

fn main_model_catalog_plan_with_mojo(
    entries: Vec<serde_json::Value>,
) -> Option<(Vec<DynamicCatalogModel>, CatalogChoicesPlan)> {
    let owned = project_main_catalog_models(&entries)
        .expect("compiled Mojo main catalog projection failed");
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
        let source = owned.get(model.source_index)?;
        choices.push(MainModelChoice {
            choice: prodex_provider_core::ProviderModelChoice::Model(model.id),
            label: model.label,
            efforts: (!model.efforts.is_empty()).then_some(model.efforts.clone()),
            aliases: source.aliases.clone(),
            default_effort: model.default_effort,
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
        .map(|choice| super_prompt::super_prompt_model_choice_label(&choice.choice, &choice.label))
        .collect::<Vec<_>>();
    let model_choices = models
        .iter()
        .map(|choice| choice.choice.clone())
        .collect::<Vec<_>>();
    let selected = super_prompt::super_prompt_model_choice_index(&model_choices, current_model);
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

    #[test]
    fn dynamic_catalog_defaults_follow_mojo_validation() {
        let choices = main_model_choices_from_catalog(vec![
            serde_json::json!({
                "slug": "first",
                "display_name": "First",
                "supported_reasoning_levels": [{"effort": " LOW "}],
                "default_reasoning_level": " low ",
                "aliases": ["first-alias"],
            }),
            serde_json::json!({
                "slug": "second",
                "display_name": "Second",
                "supported_reasoning_levels": [{"effort": "medium"}],
                "default_reasoning_level": "high",
            }),
        ])
        .expect("dynamic catalog should produce choices");
        assert_eq!(choices[1].default_effort.as_deref(), Some("low"));
        assert_eq!(choices[1].aliases, ["first-alias"]);
        assert_eq!(choices[2].default_effort, None);
    }
}

#[cfg(test)]
mod compiled_main_catalog_insertion_caller_tests {
    use super::*;

    fn choice(kind: prodex_provider_core::ProviderModelChoice) -> MainModelChoice {
        MainModelChoice {
            choice: kind,
            label: "fixture".into(),
            efforts: None,
            aliases: Vec::new(),
            default_effort: None,
        }
    }

    #[test]
    fn compiled_main_catalog_positions_remembered_model_before_custom() {
        let choices = vec![
            choice(prodex_provider_core::ProviderModelChoice::ProviderDefault),
            choice(prodex_provider_core::ProviderModelChoice::Model(
                "雪-model".into(),
            )),
            choice(prodex_provider_core::ProviderModelChoice::Custom),
        ];
        assert_eq!(
            main_model_choice_insertion_index(
                &choices,
                CatalogInsertionPlacement::CurrentBeforeCustom,
            ),
            2,
        );
        assert_eq!(
            main_model_choice_insertion_index(
                &choices,
                CatalogInsertionPlacement::BundledAfterDefault,
            ),
            1,
        );
        let no_custom = vec![
            choice(prodex_provider_core::ProviderModelChoice::ProviderDefault),
            choice(prodex_provider_core::ProviderModelChoice::Model("a".into())),
        ];
        assert_eq!(
            main_model_choice_insertion_index(
                &no_custom,
                CatalogInsertionPlacement::CurrentBeforeCustom,
            ),
            2,
        );
    }

    #[test]
    fn compiled_main_catalog_places_bundled_models_after_first_provider_default() {
        let choices = vec![
            choice(prodex_provider_core::ProviderModelChoice::Model(
                "first".into(),
            )),
            choice(prodex_provider_core::ProviderModelChoice::ProviderDefault),
            choice(prodex_provider_core::ProviderModelChoice::Model(
                "second".into(),
            )),
            choice(prodex_provider_core::ProviderModelChoice::Custom),
        ];
        assert_eq!(
            main_model_choice_insertion_index(
                &choices,
                CatalogInsertionPlacement::BundledAfterDefault,
            ),
            2,
        );
        let no_default = choices
            .into_iter()
            .filter(|choice| {
                !matches!(
                    choice.choice,
                    prodex_provider_core::ProviderModelChoice::ProviderDefault
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            main_model_choice_insertion_index(
                &no_default,
                CatalogInsertionPlacement::BundledAfterDefault,
            ),
            0,
        );
    }
}

#[cfg(test)]
#[path = "super_main_catalog/projection_tests.rs"]
mod projection_tests;

#[cfg(test)]
#[path = "super_main_catalog/lifecycle_tests.rs"]
mod lifecycle_tests;
