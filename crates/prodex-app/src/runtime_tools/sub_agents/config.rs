use anyhow::{Result, bail};
use prodex_cli::{SubAgentConfig, SuperLaunchTarget};
use prodex_mojo_core::sub_agent_policy::{
    ConfigReasoningState, ConfigUrlState, ConfigValidationAction, config_validation_plan,
};
use prodex_provider_core::{
    ProviderId, ProviderModelReasoningError, provider_model_reasoning_resolution,
    provider_model_spec,
};

use super::ResolvedSuperSubAgent;

pub(crate) fn resolve_super_sub_agent_config(
    config: SubAgentConfig,
    target: SuperLaunchTarget,
) -> Result<ResolvedSuperSubAgent> {
    let provider = config.provider;
    let model_input = config.model.as_deref();
    let model = model_input.map(|model| {
        provider_model_spec(provider, model)
            .map(|spec| spec.id.to_string())
            .unwrap_or_else(|| model.to_string())
    });
    let reasoning = match config.model_reasoning_effort {
        None => ConfigReasoningState::Absent,
        Some(effort) => match provider_model_reasoning_resolution(
            provider,
            model.as_deref(),
            Some(effort.as_str()),
        ) {
            Ok(_) => ConfigReasoningState::Valid,
            Err(ProviderModelReasoningError::UnsupportedEffort) => {
                ConfigReasoningState::Unsupported
            }
            Err(ProviderModelReasoningError::InvalidCatalog) => {
                ConfigReasoningState::InvalidCatalog
            }
        },
    };
    let parsed_url = config.url.as_deref().map(prodex_cli::parse_sub_agent_url);
    let url = match parsed_url.as_ref() {
        None => ConfigUrlState::Absent,
        Some(Ok(_)) => ConfigUrlState::Valid,
        Some(Err(_)) => ConfigUrlState::Invalid,
    };
    match config_validation_plan(model_input, reasoning, url, provider == ProviderId::Local)
        .expect("Mojo sub-agent configuration policy returned invalid output")
    {
        ConfigValidationAction::Valid => {}
        ConfigValidationAction::ModelNonempty => bail!("--sub-agent-model must be nonempty"),
        ConfigValidationAction::InvalidReasoningCatalog => {
            bail!("provider model reasoning catalog is invalid")
        }
        ConfigValidationAction::UnsupportedReasoning => {
            let effort = config
                .model_reasoning_effort
                .expect("unsupported reasoning requires an explicit effort");
            let effort_model = model
                .as_deref()
                .or_else(|| {
                    prodex_provider_core::provider_runtime_metadata(provider)
                        .map(|metadata| metadata.default_model)
                })
                .unwrap_or("unknown");
            bail!(
                "reasoning effort {} is unsupported for {} model {}; choose a catalogued effort or omit the explicit effort",
                effort.as_str(),
                provider.label(),
                effort_model
            );
        }
        ConfigValidationAction::InvalidUrl => {
            let error = parsed_url
                .as_ref()
                .and_then(|result| result.as_ref().err())
                .expect("invalid URL policy requires a parser error");
            bail!("{error}");
        }
        ConfigValidationAction::LocalRequiresUrl => {
            bail!("local sub-agent provider requires --sub-agent-url");
        }
        ConfigValidationAction::NonLocalRejectsUrl => {
            bail!("--sub-agent-url is only supported with the local sub-agent provider");
        }
    }
    let url = parsed_url.transpose().map_err(anyhow::Error::msg)?;

    Ok(ResolvedSuperSubAgent {
        provider,
        model,
        effort: config.model_reasoning_effort,
        url,
        max_concurrency: config.max_concurrency,
        target,
        presidio_enabled: false,
        required_tools: Vec::new(),
        recursion_disabled: true,
    })
}
