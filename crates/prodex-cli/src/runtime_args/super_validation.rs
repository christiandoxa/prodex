use super::SuperArgs;
use prodex_provider_core::ProviderId;

pub(super) fn validate_super_mode_compatibility(args: &SuperArgs) -> Result<(), String> {
    validate(args, prodex_mojo_core::launch::SuperValidationScope::Full)
}

pub(super) fn validate_sub_agent_flags(args: &SuperArgs) -> Result<(), String> {
    validate(
        args,
        prodex_mojo_core::launch::SuperValidationScope::SubAgentOnly,
    )
}

fn validate(
    args: &SuperArgs,
    scope: prodex_mojo_core::launch::SuperValidationScope,
) -> Result<(), String> {
    let sub_agent_model_nonempty = args
        .sub_agent_model
        .as_deref()
        .map(|model| {
            prodex_mojo_core::sub_agent_policy::model_nonempty(model)
                .map_err(|_| "sub-agent model validation failed".to_string())
        })
        .transpose()?
        .unwrap_or(false);
    let provider_url_violation = prodex_mojo_core::sub_agent_policy::provider_url_violation(
        args.sub_agent_provider == Some(ProviderId::Local),
        args.sub_agent_url.is_some(),
    )
    .map_err(|_| "sub-agent provider URL policy failed".to_string())?;
    let input = prodex_mojo_core::launch::SuperValidationInput {
        auto_rotate: args.auto_rotate,
        no_auto_rotate: args.no_auto_rotate,
        presidio: args.presidio,
        no_presidio: args.no_presidio,
        required_presidio: args
            .required_tools
            .contains(&prodex_optional_tools::OptionalToolId::Presidio),
        provider: args.provider.is_some(),
        url: args.url.is_some(),
        base_url: args.base_url.is_some(),
        api_key: args.api_key.is_some(),
        local_context_window: args.local_context_window.is_some(),
        local_auto_compact_token_limit: args.local_auto_compact_token_limit.is_some(),
        sub_agent: args.sub_agent,
        no_sub_agent: args.no_sub_agent,
        sub_agent_provider: args.sub_agent_provider.is_some(),
        sub_agent_model: args.sub_agent_model.is_some(),
        sub_agent_model_nonempty,
        sub_agent_reasoning_effort: args.sub_agent_model_reasoning_effort.is_some(),
        sub_agent_url: args.sub_agent_url.is_some(),
        sub_agent_max_concurrency: args.sub_agent_max_concurrency.is_some(),
        provider_url_violation,
    };
    let codex_argument = args.codex_args.first().map(|argument| argument.to_str());
    let violation =
        prodex_mojo_core::launch::plan_super_validation(input, codex_argument.as_slice(), scope)
            .map_err(|_| "Super argument validation failed".to_string())?;
    match violation {
        None => Ok(()),
        Some(violation) => Err(match violation {
            prodex_mojo_core::launch::SuperValidationViolation::SubAgentConflict => {
                "--sub-agent conflicts with --no-sub-agent"
            }
            prodex_mojo_core::launch::SuperValidationViolation::SubAgentDetailsRequireEnable => {
                "sub-agent detail flags require explicit --sub-agent"
            }
            prodex_mojo_core::launch::SuperValidationViolation::SubAgentModelEmpty => {
                "--sub-agent-model must be nonempty"
            }
            prodex_mojo_core::launch::SuperValidationViolation::LocalSubAgentRequiresUrl => {
                "local sub-agent provider requires --sub-agent-url"
            }
            prodex_mojo_core::launch::SuperValidationViolation::SubAgentUrlRequiresLocal => {
                "--sub-agent-url requires --sub-agent-provider local"
            }
            prodex_mojo_core::launch::SuperValidationViolation::AutoRotateConflict => {
                "--auto-rotate conflicts with --no-auto-rotate"
            }
            prodex_mojo_core::launch::SuperValidationViolation::PresidioConflict => {
                "--presidio conflicts with --no-presidio"
            }
            prodex_mojo_core::launch::SuperValidationViolation::NoPresidioRequiresPresidioTool => {
                "--no-presidio conflicts with --require-tool presidio"
            }
            prodex_mojo_core::launch::SuperValidationViolation::ProviderUrlConflict => {
                "--provider conflicts with --url"
            }
            prodex_mojo_core::launch::SuperValidationViolation::BaseUrlUrlConflict => {
                "--base-url conflicts with --url"
            }
            prodex_mojo_core::launch::SuperValidationViolation::ApiKeyRequiresProvider => {
                "--api-key requires --provider"
            }
            prodex_mojo_core::launch::SuperValidationViolation::ContextWindowRequiresProviderOrUrl => {
                "context-window options require --provider or --url"
            }
            prodex_mojo_core::launch::SuperValidationViolation::SubAgentUnsupportedWithDesktop => {
                "--sub-agent is unsupported with the Codex Desktop frontend"
            }
        }
        .to_string()),
    }
}
