use super::{SuperArgs, SuperCliAgent};
use prodex_provider_core::ProviderId;

pub(super) fn validate_super_mode_compatibility(args: &SuperArgs) -> Result<(), String> {
    validate_sub_agent_flags(args)?;
    if args.auto_rotate && args.no_auto_rotate {
        return Err("--auto-rotate conflicts with --no-auto-rotate".to_string());
    }
    if args.presidio && args.no_presidio {
        return Err("--presidio conflicts with --no-presidio".to_string());
    }
    if args.no_presidio
        && args
            .required_tools
            .contains(&prodex_optional_tools::OptionalToolId::Presidio)
    {
        return Err("--no-presidio conflicts with --require-tool presidio".to_string());
    }
    if args.provider.is_some() && args.url.is_some() {
        return Err("--provider conflicts with --url".to_string());
    }
    if args.base_url.is_some() && args.url.is_some() {
        return Err("--base-url conflicts with --url".to_string());
    }
    if args.api_key.is_some() && args.provider.is_none() {
        return Err("--api-key requires --provider".to_string());
    }
    if (args.local_context_window.is_some() || args.local_auto_compact_token_limit.is_some())
        && args.provider.is_none()
        && args.url.is_none()
    {
        return Err("context-window options require --provider or --url".to_string());
    }
    if args.harness.is_some() && args.provider.is_none() && args.url.is_none() {
        return Err("--harness requires --provider or --url".to_string());
    }
    if args.harness.is_some() && args.cli.is_some_and(|agent| agent != SuperCliAgent::Codex) {
        return Err("--harness is only supported with the Codex CLI bridge".to_string());
    }
    if args.presidio && matches!(args.cli, Some(SuperCliAgent::Kiro | SuperCliAgent::Agy)) {
        return Err("--presidio is unsupported for native Kiro or Antigravity".to_string());
    }
    if args.sub_agent && args.cli.is_some_and(|agent| agent != SuperCliAgent::Codex) {
        return Err(
            "--sub-agent is supported only on the Codex Super bridge, not native CLI launches"
                .to_string(),
        );
    }
    if args.sub_agent
        && args
            .codex_args
            .first()
            .is_some_and(|argument| argument == "gui")
    {
        return Err("--sub-agent is unsupported with the Codex Desktop frontend".to_string());
    }
    Ok(())
}

pub(super) fn validate_sub_agent_flags(args: &SuperArgs) -> Result<(), String> {
    if args.sub_agent && args.no_sub_agent {
        return Err("--sub-agent conflicts with --no-sub-agent".to_string());
    }
    if !args.sub_agent
        && (args.sub_agent_provider.is_some()
            || args.sub_agent_model.is_some()
            || args.sub_agent_model_reasoning_effort.is_some()
            || args.sub_agent_url.is_some())
    {
        return Err("sub-agent detail flags require explicit --sub-agent".to_string());
    }
    if args
        .sub_agent_model
        .as_deref()
        .is_some_and(|model| model.trim().is_empty())
    {
        return Err("--sub-agent-model must be nonempty".to_string());
    }
    let provider = args.sub_agent_provider.unwrap_or(ProviderId::OpenAi);
    if args.sub_agent_url.is_some() && provider != ProviderId::Local {
        return Err("--sub-agent-url requires --sub-agent-provider local".to_string());
    }
    if args.sub_agent
        && provider == ProviderId::Local
        && args.sub_agent_url.is_none()
        && args.url.is_none()
    {
        return Err(
            "local sub-agent provider requires --sub-agent-url or an unambiguous main --url"
                .to_string(),
        );
    }
    Ok(())
}
