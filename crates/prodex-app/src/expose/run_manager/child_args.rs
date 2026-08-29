use super::SuperArgs;
use std::ffi::OsString;

pub(super) fn build_super_child_args(args: &SuperArgs) -> Vec<OsString> {
    // Keep the child on the normal `prodex s` lifecycle; usage-limit recovery belongs there.
    // Expose is a transport for Super, not a second permission model. Keep
    // the supported Codex full-access launch flag explicit in the child argv;
    // Super's normal conversion also forces this invariant.
    let mut output = vec![OsString::from("s"), OsString::from("--full-access")];
    append_profile_args(&mut output, args);
    append_sub_agent_args(&mut output, args);
    append_tool_args(&mut output, args);
    append_target_args(&mut output, args);
    let codex_frontend = matches!(args.cli, None | Some(prodex_cli::SuperCliAgent::Codex));
    if codex_frontend {
        output.extend(args.codex_features.to_codex_config_args());
    }
    if codex_frontend {
        output.extend(args.codex_args.iter().cloned());
    } else {
        let mut native_args = args.codex_args.clone();
        for key in ["model_provider", "model_reasoning_effort"] {
            while crate::app_commands::runtime_launch::remove_first_codex_config_override_pair(
                &mut native_args,
                key,
            ) {}
        }
        output.extend(native_args);
    }
    if codex_frontend {
        output.extend([OsString::from("exec"), OsString::from("-")]);
    }
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

#[cfg(test)]
mod tests {
    use super::build_super_child_args;
    use std::ffi::OsString;

    #[test]
    fn native_frontend_drops_generated_codex_config_overrides() {
        let crate::Commands::Super(mut args) = crate::parse_cli_command_from([
            "prodex",
            "s",
            "--cli",
            "gemini",
            "--provider",
            "gemini",
        ])
        .expect("native Super args should parse") else {
            panic!("expected Super command");
        };
        args.codex_args = vec![
            OsString::from("-c"),
            OsString::from("model_provider=\"gemini\""),
            OsString::from("--config"),
            OsString::from("model_reasoning_effort=\"max\""),
            OsString::from("--prompt"),
            OsString::from("review"),
        ];

        let child_args = build_super_child_args(&args);

        assert_eq!(child_args.first().and_then(|arg| arg.to_str()), Some("s"));
        assert!(child_args.iter().any(|arg| arg == "--cli"));
        assert!(child_args.iter().any(|arg| arg == "--prompt"));
        assert!(child_args.iter().any(|arg| arg == "review"));
        assert!(
            !child_args
                .iter()
                .any(|arg| arg == "-c" || arg == "--config")
        );
        assert!(
            !child_args
                .iter()
                .any(|arg| { arg.to_string_lossy().contains("model_reasoning_effort") })
        );
        assert!(!child_args.iter().any(|arg| arg == "exec" || arg == "-"));
    }
}
