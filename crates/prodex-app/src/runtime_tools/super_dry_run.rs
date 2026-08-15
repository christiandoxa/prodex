use super::*;

pub(crate) fn handle_super_runtime_tools_dry_run(
    args: SuperArgs,
    presidio: bool,
    sub_agent: Option<&ResolvedSuperSubAgent>,
) -> Result<()> {
    let mut args = args;
    crate::app_commands::runtime_launch::resolve_super_dry_run_main_agent(&mut args)?;
    let args =
        crate::app_commands::runtime_launch::resolved_super_runtime_tool_args(args, presidio);
    if let Some(base_url) = args.base_url.as_deref() {
        validate_credential_free_http_url(base_url, "runtime upstream base URL")?;
    }
    let selected_tools = args.selected_tool_set();
    let required_tools = args.required_tool_set();
    let presidio_enabled =
        args.presidio || selected_tools.contains(prodex_optional_tools::OptionalToolId::Presidio);
    let tool_plan = resolve_runtime_optional_tool_plan(&selected_tools, &required_tools)?;
    let codex_args = args.codex_args_with_feature_overrides();
    let (_, codex_args) = extract_prodex_dry_run_flag(&codex_args);
    let (codex_args, include_code_review) =
        prepare_codex_launch_args(&codex_args, args.full_access);
    let codex_args = if args.super_mode {
        trusted_workspace_codex_args(&std::env::current_dir()?, &codex_args)
    } else {
        codex_args
    };
    let codex_args = redact_super_session_args(&codex_args);
    let model_provider_override = codex_cli_config_override_value(&codex_args, "model_provider");
    let profile_v2_name = codex_cli_profile_v2_name(&codex_args);
    let model_context_window_tokens = runtime_launch_cli_model_context_window_tokens(&codex_args);
    let gemini_thinking_budget_tokens =
        runtime_launch_cli_gemini_thinking_budget_tokens(&codex_args);
    let resolved_harness = prodex_provider_core::resolve_harness_mode(args.harness, None);
    let request = RuntimeLaunchRequest {
        profile: args.profile.as_deref(),
        allow_auto_rotate: !args.no_auto_rotate,
        auto_redeem: args.auto_redeem,
        skip_quota_check: args.skip_quota_check,
        base_url: args.base_url.as_deref(),
        upstream_no_proxy: args.no_proxy,
        include_code_review,
        smart_context_enabled: args.smart_context,
        presidio_redaction_enabled: presidio_enabled,
        model_context_window_tokens,
        gemini_thinking_budget_tokens,
        force_runtime_proxy: false,
        model_provider_override: model_provider_override.as_deref(),
        profile_v2_name: profile_v2_name.as_deref(),
        external_provider: args
            .external_provider
            .map(crate::SuperExternalProvider::as_str),
        external_provider_api_key: args.external_provider_api_key.as_deref(),
    };
    let mut extra_report = String::from("Optional tools:");
    for activation in &tool_plan.activations {
        extra_report.push_str(&format!(
            "\n  {}: resolved (activation deferred until launch)",
            activation.tool.descriptor.id
        ));
    }
    for unavailable in &tool_plan.unavailable {
        extra_report.push_str(&format!(
            "\n  {}: skipped ({})",
            unavailable.id,
            redaction::redaction_redact_secret_like_text(&unavailable.detail)
        ));
    }
    if selected_tools.contains(prodex_optional_tools::OptionalToolId::Presidio) {
        extra_report.push_str(&format!(
            "\n  presidio: {}",
            if presidio_enabled {
                "requested"
            } else {
                "disabled"
            }
        ));
    }
    extra_report.push_str("\nDry run: optional overlays and services are not started.\n");
    if let Some(sub_agent) = sub_agent {
        extra_report.push_str(&render_sub_agent_dry_run_report(sub_agent));
    } else {
        extra_report.push_str(&render_sub_agent_disabled_dry_run_report(presidio_enabled));
    }
    print_runtime_launch_dry_run(
        "optional-tools",
        request,
        RuntimeLaunchDryRunChild::Caveman { codex_args },
        Some(resolved_harness),
        Some(&extra_report),
    )
}
