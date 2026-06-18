use super::*;

#[test]
fn quota_watch_respects_once_and_raw_modes() {
    let once_args = QuotaArgs {
        profile: Some("main".to_string()),
        all: false,
        detail: false,
        raw: false,
        watch: false,
        once: true,
        auth: None,
        provider: None,
        base_url: None,
    };
    assert!(!quota_watch_enabled(&once_args));

    let raw_args = QuotaArgs {
        raw: true,
        watch: true,
        once: false,
        ..once_args
    };
    assert!(!quota_watch_enabled(&raw_args));
}

#[test]
fn quota_command_accepts_auth_filter_for_all_profiles() {
    let command = parse_cli_command_from(["prodex", "quota", "--all", "--auth", "no-auth"])
        .expect("quota command");
    let Commands::Quota(args) = command else {
        panic!("expected quota command");
    };
    assert_eq!(args.auth.as_deref(), Some("no-auth"));
}

#[test]
fn quota_command_accepts_provider_filter_for_all_profiles() {
    let command = parse_cli_command_from(["prodex", "quota", "--all", "--provider", "openai"])
        .expect("quota command");
    let Commands::Quota(args) = command else {
        panic!("expected quota command");
    };
    assert_eq!(args.provider.as_deref(), Some("openai"));
}

#[test]
fn bare_prodex_defaults_to_run_command() {
    let command = parse_cli_command_from(["prodex"]).expect("bare prodex should parse");
    let Commands::Run(args) = command else {
        panic!("expected run command");
    };
    assert!(args.profile.is_none());
    assert!(!args.auto_rotate);
    assert!(!args.no_auto_rotate);
    assert!(!args.skip_quota_check);
    assert!(args.base_url.is_none());
    assert!(args.codex_args.is_empty());
}

#[test]
fn cleanup_command_does_not_default_to_run() {
    let command = parse_cli_command_from(["prodex", "cleanup"]).expect("cleanup command");
    assert!(matches!(command, Commands::Cleanup(_)));
}

#[test]
fn logout_command_accepts_positional_profile_name() {
    let command = parse_cli_command_from(["prodex", "logout", "second"]).expect("logout command");
    let Commands::Logout(args) = command else {
        panic!("expected logout command");
    };
    assert_eq!(args.profile_name.as_deref(), Some("second"));
    assert_eq!(args.selected_profile(), Some("second"));
    assert!(args.profile.is_none());
}

#[test]
fn profile_remove_command_accepts_all_flag() {
    let command =
        parse_cli_command_from(["prodex", "profile", "remove", "--all"]).expect("remove command");
    let Commands::Profile(ProfileCommands::Remove(args)) = command else {
        panic!("expected profile remove command");
    };
    assert!(args.all);
    assert!(args.name.is_none());
    assert!(!args.delete_home);
}

#[test]
fn bare_prodex_accepts_run_options_before_codex_args() {
    let command =
        parse_cli_command_from(["prodex", "--profile", "second", "exec", "review this repo"])
            .expect("bare prodex should accept run options");
    let Commands::Run(args) = command else {
        panic!("expected run command");
    };
    assert_eq!(args.profile.as_deref(), Some("second"));
    assert_eq!(
        args.codex_args,
        vec![OsString::from("exec"), OsString::from("review this repo")]
    );
}

#[test]
fn caveman_command_accepts_passthrough_args() {
    let command = parse_cli_command_from([
        "prodex",
        "caveman",
        "--profile",
        "main",
        "exec",
        "review this repo",
    ])
    .expect("caveman command should parse");
    let Commands::Caveman(args) = command else {
        panic!("expected caveman command");
    };
    assert_eq!(args.profile.as_deref(), Some("main"));
    assert_eq!(
        args.codex_args,
        vec![OsString::from("exec"), OsString::from("review this repo")]
    );
}

#[test]
fn super_command_parses_as_distinct_subcommand_and_expands_to_full_super_prefix_stack() {
    let command = parse_cli_command_from([
        "prodex",
        "super",
        "--profile",
        "main",
        "exec",
        "review this repo",
    ])
    .expect("super command should parse");
    let Commands::Super(args) = command else {
        panic!("expected super command");
    };
    assert_eq!(args.profile.as_deref(), Some("main"));

    let args = args.into_caveman_args();
    assert_eq!(args.profile.as_deref(), Some("main"));
    assert!(args.full_access);
    assert_eq!(
        args.codex_args,
        vec![
            OsString::from("rtk"),
            OsString::from("sqz"),
            OsString::from("tokensavior"),
            OsString::from("clawcompactor"),
            OsString::from("exec"),
            OsString::from("review this repo")
        ]
    );
}

#[test]
fn super_command_accepts_s_alias() {
    let command = parse_cli_command_from([
        "prodex",
        "s",
        "--profile",
        "main",
        "exec",
        "review this repo",
    ])
    .expect("super alias command should parse");
    let Commands::Super(args) = command else {
        panic!("expected super command");
    };
    assert_eq!(args.profile.as_deref(), Some("main"));
    assert_eq!(
        args.codex_args,
        vec![OsString::from("exec"), OsString::from("review this repo")]
    );
}

#[test]
fn super_command_url_keeps_v1_path_when_provided() {
    let command = parse_cli_command_from([
        "prodex",
        "super",
        "--url",
        "http://host.docker.internal:11434/v1/",
        "exec",
        "review this repo",
    ])
    .expect("super local provider command should parse");
    let Commands::Super(args) = command else {
        panic!("expected super command");
    };

    let args = args.into_caveman_args();
    let rendered = args
        .codex_args
        .iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    assert!(
        rendered.contains(
            &"model_providers.prodex-local.base_url=\"http://host.docker.internal:11434/v1\""
                .to_string()
        )
    );
}

#[test]
fn super_command_url_accepts_local_context_overrides() {
    let command = parse_cli_command_from([
        "prodex",
        "super",
        "--url",
        "http://127.0.0.1:8131",
        "--context-window",
        "32768",
        "--auto-compact-token-limit",
        "30000",
        "exec",
        "review this repo",
    ])
    .expect("super local provider command should parse");
    let Commands::Super(args) = command else {
        panic!("expected super command");
    };

    let args = args.into_caveman_args();
    let rendered = args
        .codex_args
        .iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    assert!(rendered.contains(&"model_context_window=32768".to_string()));
    assert!(rendered.contains(&"model_auto_compact_token_limit=30000".to_string()));
}

#[test]
fn quota_watch_defaults_to_live_refresh_for_regular_views() {
    let profile_args = QuotaArgs {
        profile: Some("main".to_string()),
        all: false,
        detail: false,
        raw: false,
        watch: false,
        once: false,
        auth: None,
        provider: None,
        base_url: None,
    };
    assert!(quota_watch_enabled(&profile_args));

    let overview_args = QuotaArgs {
        all: true,
        ..profile_args
    };
    assert!(quota_watch_enabled(&overview_args));
}

#[test]
fn quota_command_accepts_once_flag() {
    let command = parse_cli_command_from(["prodex", "quota", "--once"]).expect("quota command");
    let Commands::Quota(args) = command else {
        panic!("expected quota command");
    };
    assert!(args.once);
    assert!(!quota_watch_enabled(&args));
}

#[test]
fn audit_command_accepts_filters_and_json() {
    let command = parse_cli_command_from([
        "prodex",
        "audit",
        "--tail",
        "50",
        "--component",
        "profile",
        "--action",
        "use",
        "--json",
    ])
    .expect("audit command");
    let Commands::Audit(args) = command else {
        panic!("expected audit command");
    };
    assert_eq!(args.tail, 50);
    assert_eq!(args.component.as_deref(), Some("profile"));
    assert_eq!(args.action.as_deref(), Some("use"));
    assert!(args.json);
}

#[test]
fn bare_prodex_with_codex_args_defaults_to_run_command() {
    let command = parse_cli_command_from(["prodex", "exec", "review this repo"])
        .expect("bare prodex codex args should parse");
    let Commands::Run(args) = command else {
        panic!("expected run command");
    };
    assert_eq!(
        args.codex_args,
        vec![OsString::from("exec"), OsString::from("review this repo")]
    );
}

#[test]
fn session_command_does_not_default_to_run() {
    let command = parse_cli_command_from(["prodex", "session", "list"]).expect("session command");
    assert!(matches!(
        command,
        Commands::Session(SessionCommands::List(_))
    ));
}

#[test]
fn logout_command_accepts_profile_flag() {
    let command = parse_cli_command_from(["prodex", "logout", "--profile", "second"])
        .expect("logout command");
    let Commands::Logout(args) = command else {
        panic!("expected logout command");
    };
    assert_eq!(args.profile.as_deref(), Some("second"));
    assert_eq!(args.selected_profile(), Some("second"));
    assert!(args.profile_name.is_none());
}

#[test]
fn profile_remove_command_accepts_profile_name() {
    let command =
        parse_cli_command_from(["prodex", "profile", "remove", "main"]).expect("remove command");
    let Commands::Profile(ProfileCommands::Remove(args)) = command else {
        panic!("expected profile remove command");
    };
    assert!(!args.all);
    assert_eq!(args.name.as_deref(), Some("main"));
    assert!(!args.delete_home);
}

#[test]
fn launch_commands_accept_no_proxy_flag() {
    let run = parse_cli_command_from(["prodex", "run", "--no-proxy", "exec", "hello"])
        .expect("run no-proxy should parse");
    let Commands::Run(args) = run else {
        panic!("expected run command");
    };
    assert!(args.no_proxy);

    let caveman = parse_cli_command_from(["prodex", "caveman", "--no-proxy", "exec", "hello"])
        .expect("caveman no-proxy should parse");
    let Commands::Caveman(args) = caveman else {
        panic!("expected caveman command");
    };
    assert!(args.no_proxy);

    let super_command = parse_cli_command_from(["prodex", "super", "--no-proxy", "exec", "hello"])
        .expect("super no-proxy should parse");
    let Commands::Super(args) = super_command else {
        panic!("expected super command");
    };
    assert!(args.no_proxy);
    assert!(args.into_caveman_args().no_proxy);

    let claude = parse_cli_command_from(["prodex", "claude", "--no-proxy", "--", "-p", "hello"])
        .expect("claude no-proxy should parse");
    let Commands::Claude(args) = claude else {
        panic!("expected claude command");
    };
    assert!(args.no_proxy);
}

#[test]
fn super_command_url_expands_to_local_openai_provider_config() {
    let command = parse_cli_command_from([
        "prodex",
        "super",
        "--url",
        "http://127.0.0.1:8131",
        "--model",
        "local/qwen",
        "exec",
        "review this repo",
    ])
    .expect("super local provider command should parse");
    let Commands::Super(args) = command else {
        panic!("expected super command");
    };
    assert_eq!(args.url.as_deref(), Some("http://127.0.0.1:8131"));
    assert_eq!(args.local_model.as_deref(), Some("local/qwen"));

    let args = args.into_caveman_args();
    assert!(args.full_access);
    assert!(args.skip_quota_check);

    let rendered = args
        .codex_args
        .iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    assert_eq!(rendered.first().map(String::as_str), Some("rtk"));
    assert_eq!(rendered.get(1).map(String::as_str), Some("sqz"));
    assert_eq!(rendered.get(2).map(String::as_str), Some("tokensavior"));
    assert_eq!(rendered.get(3).map(String::as_str), Some("clawcompactor"));
    assert!(rendered.contains(&"model_provider=\"prodex-local\"".to_string()));
    assert!(rendered.contains(&"model=\"local/qwen\"".to_string()));
    assert!(rendered.contains(
        &"model_providers.prodex-local.base_url=\"http://127.0.0.1:8131/v1\"".to_string()
    ));
    assert!(rendered.contains(&"model_providers.prodex-local.wire_api=\"responses\"".to_string()));
    assert!(
        rendered.contains(&"model_providers.prodex-local.supports_websockets=false".to_string())
    );
    assert!(rendered.contains(&"model_context_window=16384".to_string()));
    assert!(rendered.contains(&"model_auto_compact_token_limit=14000".to_string()));
    assert!(rendered.contains(&"web_search=\"disabled\"".to_string()));
    assert!(rendered.contains(&"features.js_repl=false".to_string()));
    assert!(rendered.contains(&"features.image_generation=false".to_string()));
    assert_eq!(
        &rendered[rendered.len() - 2..],
        ["exec", "review this repo"]
    );

    let (_, _, codex_args) = runtime_caveman_extract_launch_prefixes(&args.codex_args);
    assert_eq!(
        codex_cli_config_override_value(&codex_args, "model_provider").as_deref(),
        Some("prodex-local")
    );
}

#[test]
fn super_command_url_rejects_invalid_or_empty_values() {
    for url in ["", "not-a-url", "file:///tmp/model.sock", "http:///v1"] {
        let err = parse_cli_command_from(["prodex", "super", "--url", url, "exec", "hello"])
            .expect_err("invalid super local provider URL should fail");
        let message = err.to_string();
        assert!(
            message.contains("invalid --url"),
            "expected clear --url error for {url:?}, got {message}"
        );
    }
}
