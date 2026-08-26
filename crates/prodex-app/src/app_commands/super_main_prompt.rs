use super::super_prompt;
use crate::{
    AppPaths, canonical_sub_agent_efforts, canonical_sub_agent_model_choices,
    codex_cli_config_override_value,
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
    let current_model = explicit_model
        .clone()
        .or_else(|| remembered_main_model(args, provider))
        .or_else(|| default_main_model(provider));
    let model = if prompt_model_and_effort && explicit_model.is_none() {
        prompt_super_model(
            "Main-agent model",
            provider,
            current_model.as_deref(),
            super_prompt::configured_sub_agent_models(provider),
        )?
    } else {
        current_model
    };
    let remembered_effort = explicit_effort
        .is_none()
        .then(|| remembered_main_effort(args, provider))
        .flatten();
    let reasoning_effort = if let Some(explicit_effort) = explicit_effort {
        ensure_supported_effort(provider, model.as_deref(), &explicit_effort)?;
        Some(explicit_effort)
    } else if prompt_model_and_effort {
        prompt_super_reasoning_effort(
            "Main-agent reasoning effort",
            provider,
            model.as_deref(),
            remembered_effort
                .as_deref()
                .filter(|effort| {
                    effort.parse().ok().is_some_and(|effort| {
                        canonical_sub_agent_efforts(provider, model.as_deref()).contains(&effort)
                    })
                })
                .and_then(|value| value.parse().ok()),
        )?
        .map(|effort| effort.as_str().to_string())
    } else {
        remembered_effort
            .filter(|effort| ensure_supported_effort(provider, model.as_deref(), effort).is_ok())
    };
    Ok((model, reasoning_effort))
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

fn remembered_main_model(
    args: &SuperArgs,
    provider: prodex_provider_core::ProviderId,
) -> Option<String> {
    remembered_main_selection(args, provider).map(|selection| selection.0)
}

fn remembered_main_effort(
    args: &SuperArgs,
    provider: prodex_provider_core::ProviderId,
) -> Option<String> {
    remembered_main_selection(args, provider).and_then(|selection| selection.1)
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
    let mut efforts = vec![("provider default".to_string(), None)];
    efforts.extend(
        canonical_sub_agent_efforts(provider, model)
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
