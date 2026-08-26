use super::SuperArgs;
use std::ffi::OsString;

pub(super) fn build_super_child_args(args: &SuperArgs) -> Vec<OsString> {
    let mut output = vec![OsString::from("s")];
    push_option(&mut output, "--profile", args.profile.as_deref());
    if args.no_auto_rotate {
        output.push(OsString::from("--no-auto-rotate"));
    }
    if args.auto_redeem {
        output.push(OsString::from("--auto-redeem"));
    }
    if args.skip_quota_check {
        output.push(OsString::from("--skip-quota-check"));
    }
    push_option(&mut output, "--base-url", args.base_url.as_deref());
    if args.no_proxy {
        output.push(OsString::from("--no-proxy"));
    }
    if args.presidio {
        output.push(OsString::from("--presidio"));
    } else if args.no_presidio {
        output.push(OsString::from("--no-presidio"));
    }
    if args.sub_agent {
        output.push(OsString::from("--sub-agent"));
        push_option(
            &mut output,
            "--sub-agent-provider",
            args.sub_agent_provider.map(|provider| provider.label()),
        );
        push_option(
            &mut output,
            "--sub-agent-model",
            args.sub_agent_model.as_deref(),
        );
        if let Some(effort) = args.sub_agent_model_reasoning_effort {
            push_option(
                &mut output,
                "--sub-agent-model-reasoning-effort",
                Some(effort.as_str()),
            );
        }
        push_option(
            &mut output,
            "--sub-agent-url",
            args.sub_agent_url.as_deref(),
        );
        if let Some(limit) = args.sub_agent_max_concurrency {
            push_option(
                &mut output,
                "--sub-agent-max-concurrency",
                Some(&limit.get().to_string()),
            );
        }
    } else if args.no_sub_agent {
        output.push(OsString::from("--no-sub-agent"));
    }
    for tool in &args.tools {
        push_option(&mut output, "--tool", Some(&tool.to_string()));
    }
    for tool in &args.required_tools {
        push_option(&mut output, "--require-tool", Some(&tool.to_string()));
    }
    push_option(&mut output, "--url", args.url.as_deref());
    if let Some(provider) = args.provider {
        push_option(&mut output, "--provider", Some(provider.as_str()));
    }
    if let Some(harness) = args.harness {
        push_option(&mut output, "--harness", Some(harness.id()));
    }
    if let Some(cli) = args.cli {
        push_option(
            &mut output,
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
    push_option(&mut output, "--model", args.local_model.as_deref());
    if let Some(value) = args.local_context_window {
        push_option(&mut output, "--context-window", Some(&value.to_string()));
    }
    if let Some(value) = args.local_auto_compact_token_limit {
        push_option(
            &mut output,
            "--auto-compact-token-limit",
            Some(&value.to_string()),
        );
    }
    output.extend(args.codex_features.to_codex_config_args());
    output.extend(args.codex_args.iter().cloned());
    output.extend([OsString::from("exec"), OsString::from("-")]);
    output
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
