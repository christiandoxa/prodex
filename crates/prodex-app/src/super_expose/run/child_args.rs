use super::*;
use prodex_cli::SuperExternalProvider;
use std::ffi::OsString;

pub(super) fn build_child_args(
    args: &SuperArgs,
) -> Result<Vec<OsString>, prodex_cli::RuntimeFeaturePlanError> {
    let mut output = vec![OsString::from("s"), OsString::from("--full-access")];
    append_profile_args(&mut output, args);
    append_sub_agent_args(&mut output, args);
    append_tool_args(&mut output, args);
    append_target_args(&mut output, args);
    append_frontend_args(&mut output, args)?;
    Ok(output)
}

fn append_profile_args(output: &mut Vec<OsString>, args: &SuperArgs) {
    push_option(output, "--profile", args.profile.as_deref());
    append_flag(output, "--no-auto-rotate", args.no_auto_rotate);
    append_flag(output, "--auto-redeem", args.auto_redeem);
    append_flag(output, "--skip-quota-check", args.skip_quota_check);
    push_option(output, "--base-url", args.base_url.as_deref());
    append_flag(output, "--no-proxy", args.no_proxy);
    if args.presidio {
        output.push(OsString::from("--presidio"));
    } else if args.no_presidio {
        output.push(OsString::from("--no-presidio"));
    }
}

fn append_sub_agent_args(output: &mut Vec<OsString>, args: &SuperArgs) {
    if !args.sub_agent {
        append_flag(output, "--no-sub-agent", args.no_sub_agent);
        return;
    }
    output.push(OsString::from("--sub-agent"));
    push_option(
        output,
        "--sub-agent-provider",
        args.sub_agent_provider.map(|provider| provider.label()),
    );
    push_option(output, "--sub-agent-model", args.sub_agent_model.as_deref());
    if let Some(effort) = args.sub_agent_model_reasoning_effort {
        push_option(
            output,
            "--sub-agent-model-reasoning-effort",
            Some(effort.as_str()),
        );
    }
    push_option(output, "--sub-agent-url", args.sub_agent_url.as_deref());
    if let Some(limit) = args.sub_agent_max_concurrency {
        push_option(
            output,
            "--sub-agent-max-concurrency",
            Some(&limit.get().to_string()),
        );
    }
}

fn append_tool_args(output: &mut Vec<OsString>, args: &SuperArgs) {
    for tool in &args.tools {
        push_option(output, "--tool", Some(&tool.to_string()));
    }
    for tool in &args.required_tools {
        push_option(output, "--require-tool", Some(&tool.to_string()));
    }
}

fn append_target_args(output: &mut Vec<OsString>, args: &SuperArgs) {
    push_option(output, "--url", args.url.as_deref());
    if let Some(provider) = args.provider {
        push_option(output, "--provider", Some(provider.as_str()));
    }
    if let Some(cli) = args.cli {
        push_option(
            output,
            "--cli",
            Some(match cli {
                prodex_cli::SuperCliAgent::Agy => "agy",
            }),
        );
    }
    push_option(output, "--model", args.local_model.as_deref());
    if let Some(value) = args.local_context_window {
        push_option(output, "--context-window", Some(&value.to_string()));
    }
    if let Some(value) = args.local_auto_compact_token_limit {
        push_option(
            output,
            "--auto-compact-token-limit",
            Some(&value.to_string()),
        );
    }
}

fn append_frontend_args(
    output: &mut Vec<OsString>,
    args: &SuperArgs,
) -> Result<(), prodex_cli::RuntimeFeaturePlanError> {
    if args.cli.is_none() {
        output.extend(args.codex_features.to_codex_config_args()?);
        output.extend(args.codex_args.iter().cloned());
        output.extend([OsString::from("exec"), OsString::from("-")]);
        return Ok(());
    }
    let mut native_args = args.codex_args.clone();
    for key in ["model_provider", "model_reasoning_effort"] {
        while crate::app_commands::runtime_launch::remove_first_codex_config_override_pair(
            &mut native_args,
            key,
        ) {}
    }
    output.extend(native_args);
    Ok(())
}

fn append_flag(output: &mut Vec<OsString>, name: &str, enabled: bool) {
    if enabled {
        output.push(OsString::from(name));
    }
}

fn push_option(output: &mut Vec<std::ffi::OsString>, name: &str, value: Option<&str>) {
    if let Some(value) = value.filter(|value| !value.is_empty()) {
        output.extend([name.into(), value.into()]);
    }
}

pub(super) fn api_key_env(args: &SuperArgs) -> Option<(&'static str, &str)> {
    let key = args.api_key.as_deref()?;
    Some(match args.provider? {
        SuperExternalProvider::Anthropic => ("ANTHROPIC_API_KEY", key),
        SuperExternalProvider::Copilot => ("GITHUB_COPILOT_API_KEY", key),
        SuperExternalProvider::DeepSeek => ("DEEPSEEK_API_KEY", key),
        SuperExternalProvider::Gemini => ("GEMINI_API_KEY", key),
        SuperExternalProvider::Kiro => return None,
    })
}
