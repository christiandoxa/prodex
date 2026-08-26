use super::SuperArgs;
use std::ffi::OsString;

pub(super) fn build_super_child_args(args: &SuperArgs) -> Vec<OsString> {
    let mut output = vec![OsString::from("s")];
    append_profile_args(&mut output, args);
    append_sub_agent_args(&mut output, args);
    append_tool_args(&mut output, args);
    append_target_args(&mut output, args);
    output.extend(args.codex_features.to_codex_config_args());
    output.extend(args.codex_args.iter().cloned());
    output.extend([OsString::from("exec"), OsString::from("-")]);
    output
}

fn append_profile_args(output: &mut Vec<OsString>, args: &SuperArgs) {
    push_option(output, "--profile", args.profile.as_deref());
    if args.no_auto_rotate {
        output.push(OsString::from("--no-auto-rotate"));
    }
    if args.auto_redeem {
        output.push(OsString::from("--auto-redeem"));
    }
    if args.skip_quota_check {
        output.push(OsString::from("--skip-quota-check"));
    }
    push_option(output, "--base-url", args.base_url.as_deref());
    if args.no_proxy {
        output.push(OsString::from("--no-proxy"));
    }
    if args.presidio {
        output.push(OsString::from("--presidio"));
    } else if args.no_presidio {
        output.push(OsString::from("--no-presidio"));
    }
}

fn append_sub_agent_args(output: &mut Vec<OsString>, args: &SuperArgs) {
    if args.sub_agent {
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
    } else if args.no_sub_agent {
        output.push(OsString::from("--no-sub-agent"));
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
    if let Some(harness) = args.harness {
        push_option(output, "--harness", Some(harness.id()));
    }
    if let Some(cli) = args.cli {
        push_option(
            output,
            "--cli",
            Some(match cli {
                prodex_cli::SuperCliAgent::Codex => "codex",
                prodex_cli::SuperCliAgent::Gemini => "gemini",
                prodex_cli::SuperCliAgent::Copilot => "copilot",
                prodex_cli::SuperCliAgent::Kiro => "kiro",
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

fn push_option(output: &mut Vec<OsString>, name: &str, value: Option<&str>) {
    if let Some(value) = value.filter(|value| !value.is_empty()) {
        output.extend([OsString::from(name), OsString::from(value)]);
    }
}

pub(super) fn expose_api_key_env(args: &SuperArgs) -> Option<(&'static str, &str)> {
    let key = args.api_key.as_deref()?;
    let env_name = match args.provider? {
        prodex_cli::SuperExternalProvider::Anthropic => "ANTHROPIC_API_KEY",
        prodex_cli::SuperExternalProvider::Copilot => "GITHUB_COPILOT_API_KEY",
        prodex_cli::SuperExternalProvider::DeepSeek => "DEEPSEEK_API_KEY",
        prodex_cli::SuperExternalProvider::Gemini => "GEMINI_API_KEY",
        prodex_cli::SuperExternalProvider::Kiro => return None,
    };
    Some((env_name, key))
}
