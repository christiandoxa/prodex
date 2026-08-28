use super::super_prompt;
use crate::{
    AppPaths, canonical_sub_agent_efforts, canonical_sub_agent_model_choices,
    codex_cli_config_override_value,
};
#[path = "super_main_catalog.rs"]
mod catalog;
#[cfg(test)]
use catalog::main_model_choices_from_catalog;
#[cfg(test)]
use catalog::openai_main_model_choices;
use catalog::{
    first_main_catalog_model, main_model_choice_is_selectable, main_model_choices,
    main_model_efforts, prompt_main_model,
};
use prodex_cli::{SubAgentReasoningEffort, SuperArgs, SuperExternalProvider};

pub(super) fn resolve_main_model_and_effort(
    args: &SuperArgs,
    provider: prodex_provider_core::ProviderId,
    prompt_model_and_effort: bool,
) -> anyhow::Result<(Option<String>, Option<String>)> {
    let explicit_model = args
        .local_model
        .clone()
        .or_else(|| codex_cli_config_override_value(&args.codex_args, "model"));
    let explicit_effort =
        codex_cli_config_override_value(&args.codex_args, "model_reasoning_effort");
    let remembered_selection = remembered_main_selection(args, provider);
    let remembered_model = explicit_model
        .is_none()
        .then(|| {
            remembered_selection
                .as_ref()
                .map(|selection| selection.0.clone())
        })
        .flatten();
    // Do not add an unknown remembered value as a synthetic picker choice: ordinary
    // Super has no model picker, so stale catalog entries must fall back.
    let model_choices = main_model_choices(provider, None);
    let current_model = explicit_model
        .clone()
        .or_else(|| {
            remembered_model.filter(|model| main_model_choice_is_selectable(&model_choices, model))
        })
        .or_else(|| {
            (provider == prodex_provider_core::ProviderId::OpenAi)
                .then(|| first_main_catalog_model(&model_choices))
                .flatten()
        })
        .or_else(|| default_main_model(provider));
    let model = if prompt_model_and_effort && explicit_model.is_none() {
        prompt_main_model("Main-agent model", provider, current_model.as_deref())?
    } else {
        current_model
    };
    let remembered_effort = explicit_effort
        .is_none()
        .then(|| remembered_effort_for_model(remembered_selection.as_ref(), model.as_deref()))
        .flatten();
    let reasoning_effort = if let Some(explicit_effort) = explicit_effort {
        ensure_supported_main_effort(provider, model.as_deref(), &explicit_effort)?;
        Some(explicit_effort)
    } else if prompt_model_and_effort {
        prompt_main_reasoning_effort(
            "Main-agent reasoning effort",
            provider,
            model.as_deref(),
            remembered_effort.as_deref().filter(|effort| {
                main_model_efforts(provider, model.as_deref())
                    .iter()
                    .any(|candidate| candidate.eq_ignore_ascii_case(effort))
            }),
        )?
        .map(|effort| effort.to_string())
    } else {
        remembered_effort
            .filter(|effort| {
                ensure_supported_main_effort(provider, model.as_deref(), effort).is_ok()
            })
            .or_else(|| default_main_effort(provider, model.as_deref()))
    };
    Ok((model, reasoning_effort))
}

fn ensure_supported_main_effort(
    provider: prodex_provider_core::ProviderId,
    model: Option<&str>,
    effort: &str,
) -> anyhow::Result<()> {
    if provider != prodex_provider_core::ProviderId::OpenAi {
        return ensure_supported_effort(provider, model, effort);
    }
    if main_model_efforts(provider, model)
        .iter()
        .any(|candidate| candidate.eq_ignore_ascii_case(effort))
    {
        Ok(())
    } else {
        Err(anyhow::anyhow!(
            "reasoning effort is unsupported for the selected model"
        ))
    }
}

pub(super) fn ensure_supported_effort(
    provider: prodex_provider_core::ProviderId,
    model: Option<&str>,
    effort: &str,
) -> anyhow::Result<()> {
    let parsed = effort
        .parse::<SubAgentReasoningEffort>()
        .map_err(|_| anyhow::anyhow!("reasoning effort is unsupported"))?;
    if canonical_sub_agent_efforts(provider, model).contains(&parsed) {
        Ok(())
    } else {
        Err(anyhow::anyhow!(
            "reasoning effort is unsupported for the selected model"
        ))
    }
}

fn remembered_main_selection(
    args: &SuperArgs,
    provider: prodex_provider_core::ProviderId,
) -> Option<(String, Option<String>)> {
    let paths = AppPaths::discover().ok()?;
    let codex_home = prodex_core::default_codex_home(&paths).ok()?;
    let mut preference_args = args.clone();
    preference_args.local_model = None;
    match provider {
        prodex_provider_core::ProviderId::OpenAi => {
            preference_args.provider = None;
            preference_args.url = None;
        }
        prodex_provider_core::ProviderId::Local => {
            preference_args.provider = None;
            preference_args.url = Some("http://127.0.0.1:11434/v1".to_string());
        }
        provider => {
            preference_args.provider = SuperExternalProvider::from_provider_id(provider);
            preference_args.url = None;
        }
    }
    if provider == prodex_provider_core::ProviderId::OpenAi
        && codex_cli_config_override_value(&preference_args.codex_args, "model_provider").is_none()
    {
        preference_args
            .codex_args
            .splice(0..0, ["-c".into(), "model_provider=\"openai\"".into()]);
    }
    let runtime_args = preference_args.into_runtime_tool_args_with_presidio(false);
    let context = crate::resolve_fresh_model_preference_context_read_only(
        &paths,
        &codex_home,
        &runtime_args.codex_args,
    )
    .ok()?;
    if let Some(selection) = context.remembered {
        return Some((selection.model, selection.reasoning_effort));
    }
    let model = crate::codex_effective_config_value(&codex_home, &runtime_args.codex_args, "model")
        .ok()
        .flatten()?;
    let effort = crate::codex_effective_config_value(
        &codex_home,
        &runtime_args.codex_args,
        "model_reasoning_effort",
    )
    .ok()
    .flatten();
    Some((model, effort))
}

fn remembered_effort_for_model(
    selection: Option<&(String, Option<String>)>,
    model: Option<&str>,
) -> Option<String> {
    let (remembered_model, effort) = selection?;
    model.filter(|selected_model| selected_model.eq_ignore_ascii_case(remembered_model))?;
    effort.clone()
}

fn default_main_model(provider: prodex_provider_core::ProviderId) -> Option<String> {
    prodex_provider_core::provider_runtime_metadata(provider)
        .map(|metadata| metadata.default_model.to_string())
        .or_else(|| {
            prodex_provider_core::provider_catalog_entries_for(provider)
                .first()
                .map(|entry| entry.id.clone())
        })
}

fn default_main_effort(
    provider: prodex_provider_core::ProviderId,
    model: Option<&str>,
) -> Option<String> {
    prodex_provider_core::provider_catalog_entry(provider, model?)
        .and_then(|entry| entry.default_reasoning_effort)
        .and_then(|effort| match effort {
            prodex_provider_core::ProviderReasoningEffort::None => Some("none"),
            prodex_provider_core::ProviderReasoningEffort::Minimal => Some("minimal"),
            prodex_provider_core::ProviderReasoningEffort::Low => Some("low"),
            prodex_provider_core::ProviderReasoningEffort::Medium => Some("medium"),
            prodex_provider_core::ProviderReasoningEffort::High => Some("high"),
            prodex_provider_core::ProviderReasoningEffort::XHigh => Some("xhigh"),
            prodex_provider_core::ProviderReasoningEffort::Max => Some("max"),
            prodex_provider_core::ProviderReasoningEffort::Ultra => Some("ultra"),
            prodex_provider_core::ProviderReasoningEffort::Unknown => None,
        })
        .map(str::to_string)
        .or_else(|| main_model_efforts(provider, model).first().cloned())
}

pub(super) fn prompt_super_model(
    title: &str,
    provider: prodex_provider_core::ProviderId,
    current_model: Option<&str>,
    configured_models: Vec<String>,
) -> anyhow::Result<Option<String>> {
    let models = if configured_models.is_empty() {
        canonical_sub_agent_model_choices(provider, current_model)
    } else {
        prodex_provider_core::resolve_provider_model_choices(
            provider,
            &configured_models,
            current_model,
        )
    };
    let choices = models
        .iter()
        .map(|choice| match choice {
            prodex_provider_core::ProviderModelChoice::ProviderDefault => {
                "provider default".to_string()
            }
            prodex_provider_core::ProviderModelChoice::Model(model) => model.clone(),
            prodex_provider_core::ProviderModelChoice::Custom => "custom model...".to_string(),
        })
        .collect::<Vec<_>>();
    let selected = current_model
        .and_then(|model| {
            models.iter().position(|choice| {
                matches!(choice, prodex_provider_core::ProviderModelChoice::Model(candidate) if candidate.eq_ignore_ascii_case(model))
            })
        })
        .unwrap_or(0);
    let selected = super_prompt::prompt_super_choice(title, &choices, selected, false)?;
    Ok(match &models[selected] {
        prodex_provider_core::ProviderModelChoice::ProviderDefault => None,
        prodex_provider_core::ProviderModelChoice::Model(model) => Some(model.clone()),
        prodex_provider_core::ProviderModelChoice::Custom => Some(super_prompt::prompt_super_text(
            "Custom model",
            current_model.unwrap_or_default(),
        )?),
    })
}

pub(super) fn prompt_super_reasoning_effort(
    title: &str,
    provider: prodex_provider_core::ProviderId,
    model: Option<&str>,
    current: Option<SubAgentReasoningEffort>,
) -> anyhow::Result<Option<SubAgentReasoningEffort>> {
    prompt_reasoning_effort(title, canonical_sub_agent_efforts(provider, model), current)
}

fn prompt_main_reasoning_effort(
    title: &str,
    provider: prodex_provider_core::ProviderId,
    model: Option<&str>,
    current: Option<&str>,
) -> anyhow::Result<Option<String>> {
    let mut efforts = vec![("provider default".to_string(), None)];
    efforts.extend(
        main_model_efforts(provider, model)
            .into_iter()
            .map(|effort| (effort.clone(), Some(effort))),
    );
    let choices = efforts
        .iter()
        .map(|(label, _)| label.clone())
        .collect::<Vec<_>>();
    let selected = efforts
        .iter()
        .position(|(_, effort)| {
            effort
                .as_deref()
                .zip(current)
                .is_some_and(|(candidate, current)| candidate.eq_ignore_ascii_case(current))
        })
        .unwrap_or(0);
    Ok(
        efforts[super_prompt::prompt_super_choice(title, &choices, selected, false)?]
            .1
            .clone(),
    )
}

fn prompt_reasoning_effort(
    title: &str,
    supported: Vec<SubAgentReasoningEffort>,
    current: Option<SubAgentReasoningEffort>,
) -> anyhow::Result<Option<SubAgentReasoningEffort>> {
    let mut efforts = vec![("provider default".to_string(), None)];
    efforts.extend(
        supported
            .into_iter()
            .map(|effort| (effort.as_str().to_string(), Some(effort))),
    );
    let choices = efforts
        .iter()
        .map(|(label, _)| label.clone())
        .collect::<Vec<_>>();
    let selected = efforts
        .iter()
        .position(|(_, effort)| *effort == current)
        .unwrap_or(0);
    Ok(efforts[super_prompt::prompt_super_choice(title, &choices, selected, false)?].1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn catalog_model(
        slug: &str,
        display_name: &str,
        priority: u64,
        efforts: &[&str],
    ) -> serde_json::Value {
        json!({
            "slug": slug,
            "display_name": display_name,
            "visibility": "list",
            "supported_in_api": true,
            "priority": priority,
            "supported_reasoning_levels": efforts.iter().map(|effort| json!({"effort": effort})).collect::<Vec<_>>(),
        })
    }

    #[test]
    fn openai_main_picker_uses_top_level_catalog_and_model_efforts() {
        let choices = main_model_choices_from_catalog(vec![
            catalog_model("gpt-5.6-luna", "GPT-5.6 Luna", 3, &["low", "medium"]),
            catalog_model("gpt-5.6-sol", "GPT-5.6 Sol", 1, &["low", "max", "ultra"]),
            catalog_model("gpt-5.6-terra", "GPT-5.6 Terra", 2, &["medium", "high"]),
            json!({"slug": "hidden", "visibility": "hide"}),
            json!({"slug": "unsupported", "supported_in_api": false}),
            catalog_model("GPT-5.6-SOL", "duplicate", 4, &["low"]),
        ])
        .unwrap();

        let models = choices
            .iter()
            .filter_map(|choice| match &choice.choice {
                prodex_provider_core::ProviderModelChoice::Model(model) => Some(model.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(models, ["gpt-5.6-sol", "gpt-5.6-terra", "gpt-5.6-luna"]);
        assert_eq!(choices[1].label, "GPT-5.6 Sol");
        assert_eq!(
            choices[1].efforts,
            Some(vec![
                "low".to_string(),
                "max".to_string(),
                "ultra".to_string()
            ])
        );
        assert_eq!(
            choices.last().map(|choice| choice.label.as_str()),
            Some("custom model...")
        );
    }

    #[test]
    fn main_and_sub_agent_catalogs_remain_independent() {
        let main = main_model_choices_from_catalog(vec![
            catalog_model("gpt-5.6-sol", "GPT-5.6 Sol", 1, &["low"]),
            catalog_model("gpt-5.6-terra", "GPT-5.6 Terra", 2, &["high"]),
            catalog_model("gpt-5.6-luna", "GPT-5.6 Luna", 3, &["medium"]),
        ])
        .unwrap();
        let mut sub_agent = Vec::new();
        super_prompt::configured_sub_agent_model_ids(
            &json!({"models": [{"slug": "gpt-5.6-sol"}, {"slug": "gpt-5.6-terra"}]}),
            &mut sub_agent,
            3,
        );

        assert!(main.iter().any(|choice| matches!(
            &choice.choice,
            prodex_provider_core::ProviderModelChoice::Model(model) if model == "gpt-5.6-luna"
        )));
        assert_eq!(sub_agent, ["gpt-5.6-sol", "gpt-5.6-terra"]);
    }

    #[test]
    fn main_catalog_keeps_all_models_and_model_specific_efforts() {
        let entries = (0..32)
            .map(|index| {
                let model = format!("fixture-model-{index:02}");
                let efforts = if index == 31 {
                    vec![
                        "minimal",
                        "low",
                        "medium",
                        "high",
                        "xhigh",
                        "max",
                        "ultra",
                        "future-depth",
                    ]
                } else {
                    vec!["medium"]
                };
                catalog_model(&model, &model, index, &efforts)
            })
            .collect();
        let choices = main_model_choices_from_catalog(entries).unwrap();
        let models = choices
            .iter()
            .filter_map(|choice| match &choice.choice {
                prodex_provider_core::ProviderModelChoice::Model(model) => Some(model.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(models.len(), 32);
        assert_eq!(models.first(), Some(&"fixture-model-00"));
        assert_eq!(models.get(5), Some(&"fixture-model-05"));
        assert_eq!(models.last(), Some(&"fixture-model-31"));
        assert_eq!(
            choices[32].efforts,
            Some(vec![
                "minimal".to_string(),
                "low".to_string(),
                "medium".to_string(),
                "high".to_string(),
                "xhigh".to_string(),
                "max".to_string(),
                "ultra".to_string(),
                "future-depth".to_string(),
            ])
        );
    }

    #[test]
    fn remembered_effort_is_not_reused_after_model_fallback_or_change() {
        let selection = ("gpt-5.6-sol".to_string(), Some("max".to_string()));
        assert_eq!(
            remembered_effort_for_model(Some(&selection), Some("GPT-5.6-SOL")),
            Some("max".to_string())
        );
        assert_eq!(
            remembered_effort_for_model(Some(&selection), Some("gpt-5.6-terra")),
            None
        );
        assert_eq!(remembered_effort_for_model(Some(&selection), None), None);
    }

    #[test]
    fn openai_main_picker_reads_the_active_models_cache() {
        let root =
            crate::test_temp_root().join(format!("prodex-main-model-cache-{}", std::process::id()));
        let codex_home = root.join("codex");
        std::fs::create_dir_all(&codex_home).unwrap();
        let _env_lock = crate::test_support::TestEnvVarGuard::lock();
        let _prodex_home = crate::test_support::TestEnvVarGuard::set(
            "PRODEX_HOME",
            root.join("prodex").to_str().unwrap(),
        );
        let _codex_home = crate::test_support::TestEnvVarGuard::set(
            "PRODEX_SHARED_CODEX_HOME",
            codex_home.to_str().unwrap(),
        );
        std::fs::write(
            codex_home.join("models_cache.json"),
            json!({
                "client_version": "0.150.1",
                "models": [
                    catalog_model("gpt-5.6-terra", "GPT-5.6 Terra", 2, &["high"]),
                    catalog_model("gpt-5.6-sol", "GPT-5.6 Sol", 1, &["max"]),
                    catalog_model("gpt-5.6-luna", "GPT-5.6 Luna", 3, &["medium"]),
                ]
            })
            .to_string(),
        )
        .unwrap();

        let choices = openai_main_model_choices().unwrap();
        let models = choices
            .iter()
            .filter_map(|choice| match &choice.choice {
                prodex_provider_core::ProviderModelChoice::Model(model) => Some(model.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(models, ["gpt-5.6-sol", "gpt-5.6-terra", "gpt-5.6-luna"]);
        drop(_codex_home);
        drop(_prodex_home);
        drop(_env_lock);
        let _ = std::fs::remove_dir_all(root);
    }
}
