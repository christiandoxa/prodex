use super::*;

#[test]
fn super_external_providers_enable_live_web_search() {
    for provider in ["anthropic", "copilot", "deepseek", "gemini"] {
        let args = parse_super_as_runtime_tools(&[
            "prodex",
            "s",
            "--provider",
            provider,
            "--api-key",
            "test-key",
            "exec",
            "review",
        ]);
        let rendered = args
            .codex_args
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();

        assert!(
            rendered.contains(&"web_search=\"live\"".to_string()),
            "{provider} should enable live web search"
        );
    }
}

#[test]
fn super_external_providers_use_openai_responses_wire_api() {
    for provider in ["anthropic", "copilot", "deepseek", "gemini"] {
        let args = parse_super_as_runtime_tools(&[
            "prodex",
            "s",
            "--provider",
            provider,
            "--api-key",
            "test-key",
            "exec",
            "review",
        ]);
        let rendered = args
            .codex_args
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        let provider_id = args.external_provider.unwrap().model_provider_id();

        assert!(
            rendered.contains(&format!("model_provider=\"{provider_id}\"")),
            "{provider} should set its model_provider"
        );
        assert!(
            rendered.contains(&format!(
                "model_providers.{provider_id}.wire_api=\"responses\""
            )),
            "{provider} should expose OpenAI Responses wire API to Codex"
        );
        assert!(
            rendered.contains(&format!(
                "model_providers.{provider_id}.supports_websockets=false"
            )),
            "{provider} should disable websocket transport at the OpenAI bridge boundary"
        );
        assert!(
            rendered.contains(&format!(
                "model_providers.{provider_id}.requires_openai_auth=true"
            )),
            "{provider} should request only Prodex's explicit bridge credential"
        );
    }
}

#[test]
fn super_provider_short_aliases_expand_to_provider_flag() {
    for provider in ["deepseek", "gemini"] {
        let alias_args = parse_super_as_runtime_tools(&[
            "prodex",
            "s",
            provider,
            "--api-key",
            "test-key",
            "exec",
            "review",
        ]);
        let explicit_args = parse_super_as_runtime_tools(&[
            "prodex",
            "s",
            "--provider",
            provider,
            "--api-key",
            "test-key",
            "exec",
            "review",
        ]);

        assert_same_runtime_tool_args(alias_args, explicit_args);
    }
}

#[test]
fn super_command_provider_short_aliases_expand_to_provider_flag() {
    let args = parse_super_as_runtime_tools(&[
        "prodex",
        "super",
        "gemini",
        "--api-key",
        "test-key",
        "exec",
        "review",
    ]);

    assert_eq!(args.external_provider, Some(SuperExternalProvider::Gemini));
    assert_eq!(
        args.codex_args.last().and_then(|arg| arg.to_str()),
        Some("review")
    );
}

#[test]
fn login_accepts_antigravity_flag_as_passthrough_arg() {
    let command = parse_cli_command_from(["prodex", "login", "--with-antigravity"])
        .expect("login command should parse");
    let Commands::Login(args) = command else {
        panic!("expected login command");
    };

    assert_eq!(args.codex_args, vec![OsString::from("--with-antigravity")]);
}

#[test]
fn login_accepts_positional_profile_name() {
    let command = parse_cli_command_from(["prodex", "login", "main", "--device-auth"])
        .expect("named login should parse");
    let Commands::Login(args) = command else {
        panic!("expected login command");
    };

    assert_eq!(args.selected_profile(), Some("main"));
    let (profile, codex_args) = args.into_selected_profile();
    assert_eq!(profile.as_deref(), Some("main"));
    assert_eq!(codex_args, vec![OsString::from("--device-auth")]);

    let Commands::Login(args) =
        parse_cli_command_from(["prodex", "login", "--profile", "main", "--device-auth"])
            .expect("named option login should remain supported")
    else {
        panic!("expected login command");
    };
    assert_eq!(args.selected_profile(), Some("main"));
    assert_eq!(args.codex_args, vec![OsString::from("--device-auth")]);

    let Commands::Login(args) = parse_cli_command_from(["prodex", "login", "status"])
        .expect("Codex login status should remain passthrough")
    else {
        panic!("expected login command");
    };
    assert_eq!(args.selected_profile(), None);
    assert_eq!(args.codex_args, vec![OsString::from("status")]);
}

#[test]
fn super_deepseek_provider_rejects_unknown_provider() {
    assert!(parse_cli_command_from(["prodex", "s", "--provider", "unknown", "exec"]).is_err());
}

#[test]
fn gateway_provider_args_parse_as_top_level_command() {
    let command = parse_cli_command_from([
        "prodex",
        "gateway",
        "--listen",
        "127.0.0.1:4100",
        "--provider",
        "gemini",
        "--api-key",
        "test-key",
        "--smart-context",
        "--presidio",
    ])
    .expect("gateway command should parse");
    let Commands::Gateway(args) = command else {
        panic!("expected gateway command");
    };
    assert_eq!(args.listen.as_deref(), Some("127.0.0.1:4100"));
    assert_eq!(args.provider, Some(SuperExternalProvider::Gemini));
    assert_eq!(args.api_key.as_deref(), Some("test-key"));
    assert!(args.smart_context);
    assert!(args.presidio);
}

#[test]
fn super_gemini_provider_enables_native_image_generation_only_for_gemini() {
    for provider in ["anthropic", "copilot", "deepseek", "gemini"] {
        let args = parse_super_as_runtime_tools(&[
            "prodex",
            "s",
            "--provider",
            provider,
            "--api-key",
            "test-key",
            "exec",
            "draw",
        ]);
        let rendered = args
            .codex_args
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        let expected = format!("features.image_generation={}", provider == "gemini");

        assert!(
            rendered.contains(&expected),
            "{provider} image generation feature mismatch"
        );
    }
}
