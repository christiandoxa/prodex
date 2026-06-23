use super::*;
use std::ffi::OsString;
#[path = "app_server.rs"]
mod app_server;
#[path = "cleanup.rs"]
mod cleanup;
#[path = "dashboard.rs"]
mod dashboard;
#[path = "expose.rs"]
mod expose;
#[path = "external_provider.rs"]
mod external_provider;
#[path = "redeem.rs"]
mod redeem;
#[path = "shortcuts.rs"]
mod shortcuts;
fn parse_super_as_caveman(args: &[&str]) -> CavemanArgs {
    let command = parse_cli_command_from(args.iter().copied()).expect("super command should parse");
    let Commands::Super(args) = command else {
        panic!("expected super command");
    };
    args.into_caveman_args()
}
fn parse_super_as_caveman_with_presidio_preference(args: &[&str]) -> CavemanArgs {
    let command = parse_cli_command_from(args.iter().copied()).expect("super command should parse");
    let Commands::Super(args) = command else {
        panic!("expected super command");
    };
    let use_presidio = args.presidio_preference().unwrap_or(false);
    args.into_caveman_args_with_presidio(use_presidio)
}
fn os_args(args: &[&str]) -> Vec<OsString> {
    args.iter().map(OsString::from).collect()
}
fn rendered_codex_args(args: &CavemanArgs) -> Vec<String> {
    args.codex_args
        .iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect()
}
fn assert_same_caveman_args(left: CavemanArgs, right: CavemanArgs) {
    assert_eq!(left.profile, right.profile);
    assert_eq!(left.auto_rotate, right.auto_rotate);
    assert_eq!(left.no_auto_rotate, right.no_auto_rotate);
    assert_eq!(left.skip_quota_check, right.skip_quota_check);
    assert_eq!(left.full_access, right.full_access);
    assert_eq!(left.dry_run, right.dry_run);
    assert_eq!(left.base_url, right.base_url);
    assert_eq!(left.no_proxy, right.no_proxy);
    assert_eq!(left.smart_context, right.smart_context);
    assert_eq!(left.super_optimizer_overlay, right.super_optimizer_overlay);
    assert_eq!(left.external_provider, right.external_provider);
    assert_eq!(
        left.external_provider_api_key,
        right.external_provider_api_key
    );
    assert_eq!(left.memory_backend, right.memory_backend);
    assert_eq!(left.codex_args, right.codex_args);
}
#[test]
fn presidio_commands_parse_as_top_level_commands() {
    let command = parse_cli_command_from(["prodex", "presidio", "doctor", "--json"])
        .expect("presidio doctor should parse");
    assert!(matches!(
        command,
        Commands::Presidio(PresidioCommands::Doctor(PresidioDoctorArgs {
            json: true,
            ..
        }))
    ));
    let command = parse_cli_command_from([
        "prodex",
        "presidio",
        "redact",
        "--text",
        "my phone is 212-555-1234",
        "--json",
    ])
    .expect("presidio redact should parse");
    assert!(matches!(
        command,
        Commands::Presidio(PresidioCommands::Redact(PresidioRedactArgs {
            json: true,
            ..
        }))
    ));
}
#[test]
fn doctor_install_parse_as_top_level_command() {
    let command = parse_cli_command_from(["prodex", "doctor", "--install"])
        .expect("doctor install should parse");
    let Commands::Doctor(args) = command else {
        panic!("expected doctor command");
    };
    assert!(args.install);
}
#[test]
fn setup_parse_as_top_level_command() {
    let command =
        parse_cli_command_from(["prodex", "setup", "--dry-run", "--verify-assets", "--json"])
            .expect("setup should parse");
    let Commands::Setup(args) = command else {
        panic!("expected setup command");
    };
    assert!(args.dry_run);
    assert!(args.verify_assets);
    assert!(args.json);
    assert!(!should_default_cli_invocation_to_run(&os_args(&[
        "prodex", "setup",
    ])));
    let args = ["prodex", "__inspect-mcp"];
    let command = parse_cli_command_from(args).expect("inspect MCP should parse");
    assert!(matches!(command, Commands::InspectMcp(_)));
    assert!(!should_default_cli_invocation_to_run(&os_args(&args)));
}
#[test]
fn capability_list_parse_as_top_level_command() {
    let command = parse_cli_command_from(["prodex", "capability", "list", "--json"])
        .expect("capability list should parse");
    let Commands::Capability(CapabilityCommands::List(args)) = command else {
        panic!("expected capability list command");
    };
    assert!(args.json);
    assert!(!should_default_cli_invocation_to_run(&os_args(&[
        "prodex",
        "capability",
    ])));
}
#[test]
fn super_and_s_parse_to_same_default_super_behavior() {
    let super_args = parse_super_as_caveman(&["prodex", "super"]);
    let alias_args = parse_super_as_caveman(&["prodex", "s"]);
    assert_same_caveman_args(super_args, alias_args);
}
#[test]
fn super_and_s_parse_to_same_super_behavior_with_options() {
    let super_args = parse_super_as_caveman(&[
        "prodex",
        "super",
        "--profile",
        "main",
        "--no-auto-rotate",
        "--skip-quota-check",
        "--dry-run",
        "--no-proxy",
        "--url",
        "http://127.0.0.1:8131",
        "--model",
        "local-model",
        "--context-window",
        "32000",
        "--auto-compact-token-limit",
        "24000",
        "exec",
        "review",
        "--dangerously-bypass-approvals-and-sandbox",
    ]);
    let alias_args = parse_super_as_caveman(&[
        "prodex",
        "s",
        "--profile",
        "main",
        "--no-auto-rotate",
        "--skip-quota-check",
        "--dry-run",
        "--no-proxy",
        "--url",
        "http://127.0.0.1:8131",
        "--model",
        "local-model",
        "--context-window",
        "32000",
        "--auto-compact-token-limit",
        "24000",
        "exec",
        "review",
        "--dangerously-bypass-approvals-and-sandbox",
    ]);
    assert_same_caveman_args(super_args, alias_args);
}
#[test]
fn s_profile_shortcut_selects_profile() {
    let command = parse_cli_command_from(["prodex", "s", "--profile", "nama_profile"])
        .expect("s profile command should parse");
    let Commands::Super(args) = command else {
        panic!("expected super command");
    };
    assert_eq!(args.profile.as_deref(), Some("nama_profile"));
}
#[test]
fn super_default_keeps_all_super_prefixes() {
    let args = parse_super_as_caveman(&["prodex", "super", "exec", "review"]);
    assert!(args.full_access);
    assert!(args.smart_context);
    assert!(args.super_optimizer_overlay);
    assert_eq!(
        args.codex_args,
        vec![
            OsString::from("rtk"),
            OsString::from("sqz"),
            OsString::from("tokensavior"),
            OsString::from("clawcompactor"),
            OsString::from("exec"),
            OsString::from("review")
        ]
    );
}
#[test]
fn super_and_s_enable_smart_context_autopilot() {
    assert!(parse_super_as_caveman(&["prodex", "super"]).smart_context);
    assert!(parse_super_as_caveman(&["prodex", "s"]).smart_context);
}
#[test]
fn super_and_s_enable_super_optimizer_overlay() {
    assert!(parse_super_as_caveman(&["prodex", "super"]).super_optimizer_overlay);
    assert!(parse_super_as_caveman(&["prodex", "s"]).super_optimizer_overlay);
}
#[test]
fn super_omits_presidio_prefix_until_prompt_opt_in() {
    let command = parse_cli_command_from(["prodex", "super", "exec", "hello"])
        .expect("super command should parse");
    let Commands::Super(args) = command else {
        panic!("expected super command");
    };
    assert_eq!(
        args.into_caveman_args().codex_args,
        os_args(&[
            "rtk",
            "sqz",
            "tokensavior",
            "clawcompactor",
            "exec",
            "hello",
        ])
    );
}
#[test]
fn super_includes_presidio_prefix_when_opted_in() {
    let command = parse_cli_command_from(["prodex", "super", "exec", "hello"])
        .expect("super command should parse");
    let Commands::Super(args) = command else {
        panic!("expected super command");
    };
    assert_eq!(
        args.into_caveman_args_with_presidio(true).codex_args,
        os_args(&[
            "rtk",
            "sqz",
            "tokensavior",
            "clawcompactor",
            "presidio",
            "exec",
            "hello",
        ])
    );
}
#[test]
fn super_presidio_flag_enables_presidio_without_prompt() {
    let args = parse_super_as_caveman_with_presidio_preference(&[
        "prodex",
        "super",
        "--presidio",
        "exec",
        "hello",
    ]);
    assert_eq!(
        args.codex_args,
        os_args(&[
            "rtk",
            "sqz",
            "tokensavior",
            "clawcompactor",
            "presidio",
            "exec",
            "hello",
        ])
    );
}
#[test]
fn s_presidio_flag_matches_super_presidio_flag() {
    let super_args = parse_super_as_caveman_with_presidio_preference(&[
        "prodex",
        "super",
        "--presidio",
        "exec",
        "hello",
    ]);
    let alias_args = parse_super_as_caveman_with_presidio_preference(&[
        "prodex",
        "s",
        "--presidio",
        "exec",
        "hello",
    ]);
    assert_same_caveman_args(super_args, alias_args);
}
#[test]
fn super_no_presidio_flag_disables_presidio_without_prompt() {
    let args = parse_super_as_caveman_with_presidio_preference(&[
        "prodex",
        "super",
        "--no-presidio",
        "exec",
        "hello",
    ]);
    assert_eq!(
        args.codex_args,
        os_args(&[
            "rtk",
            "sqz",
            "tokensavior",
            "clawcompactor",
            "exec",
            "hello",
        ])
    );
}
#[test]
fn super_mem0_flag_selects_managed_memory_backend() {
    let command = parse_cli_command_from(["prodex", "super", "--mem0", "exec", "hello"])
        .expect("super command should parse");
    let Commands::Super(args) = command else {
        panic!("expected super command");
    };
    assert_eq!(args.mem0_preference(), Some(true));
    let caveman = args.into_caveman_args_with_choices(false, true);
    assert_eq!(caveman.memory_backend, SuperMemoryBackend::Mem0);
    assert!(caveman.codex_args.contains(&OsString::from("mem")));
}
#[test]
fn super_no_mem0_flag_selects_sqlite_memory_backend() {
    let command = parse_cli_command_from(["prodex", "super", "--no-mem0", "exec", "hello"])
        .expect("super command should parse");
    let Commands::Super(args) = command else {
        panic!("expected super command");
    };
    assert_eq!(args.mem0_preference(), Some(false));
    let caveman = args.into_caveman_args_with_choices(false, false);
    assert_eq!(caveman.memory_backend, SuperMemoryBackend::Sqlite);
    assert!(!caveman.codex_args.contains(&OsString::from("mem")));
}
#[test]
fn super_mem0_flags_conflict() {
    assert!(parse_cli_command_from(["prodex", "super", "--mem0", "--no-mem0", "exec"]).is_err());
}
#[test]
fn super_presidio_flags_conflict() {
    assert!(
        parse_cli_command_from(["prodex", "super", "--presidio", "--no-presidio", "exec"]).is_err()
    );
}
#[test]
fn super_url_sets_runtime_base_url_for_local_rewrite_proxy() {
    let args = parse_super_as_caveman(&["prodex", "super", "--url", "http://127.0.0.1:8131"]);
    assert_eq!(args.base_url.as_deref(), Some("http://127.0.0.1:8131/v1"));
}
#[test]
fn super_url_local_provider_uses_openai_responses_wire_api() {
    let args = parse_super_as_caveman(&[
        "prodex",
        "super",
        "--url",
        "http://127.0.0.1:8131",
        "--model",
        "qwen3-coder",
    ]);
    let rendered = rendered_codex_args(&args);
    assert!(rendered.contains(&"model_provider=\"prodex-local\"".to_string()));
    assert!(rendered.contains(&"model=\"qwen3-coder\"".to_string()));
    assert!(rendered.contains(&"model_providers.prodex-local.wire_api=\"responses\"".to_string()));
    assert!(
        rendered.contains(&"model_providers.prodex-local.supports_websockets=false".to_string())
    );
}
#[test]
fn super_deepseek_provider_expands_to_local_responses_adapter_config() {
    let args = parse_super_as_caveman(&[
        "prodex",
        "s",
        "--provider",
        "deepseek",
        "--model",
        "deepseek-v4-pro",
        "--api-key",
        "ds-test-key",
        "exec",
        "review",
    ]);
    assert_eq!(
        args.external_provider,
        Some(SuperExternalProvider::DeepSeek)
    );
    assert_eq!(
        args.external_provider_api_key.as_deref(),
        Some("ds-test-key")
    );
    assert_eq!(args.base_url.as_deref(), Some("https://api.deepseek.com"));
    assert!(args.skip_quota_check);
    let rendered = rendered_codex_args(&args);
    assert!(rendered.contains(&"model_provider=\"prodex-deepseek\"".to_string()));
    assert!(rendered.contains(&"model=\"deepseek-v4-pro\"".to_string()));
    assert!(rendered.contains(
        &"model_providers.prodex-deepseek.base_url=\"https://api.deepseek.com\"".to_string()
    ));
    assert!(
        rendered.contains(&"model_providers.prodex-deepseek.wire_api=\"responses\"".to_string())
    );
    assert!(rendered.contains(&"web_search=\"live\"".to_string()));
    assert!(rendered.contains(&"features.apps=false".to_string()));
    assert!(!rendered.iter().any(|arg| arg.contains("ds-test-key")));
}
#[test]
fn super_gemini_provider_expands_to_local_responses_adapter_config() {
    let args = parse_super_as_caveman(&[
        "prodex",
        "s",
        "--provider",
        "gemini",
        "--api-key",
        "gemini-test-key",
        "exec",
        "review",
    ]);
    assert_eq!(args.external_provider, Some(SuperExternalProvider::Gemini));
    assert_eq!(
        args.external_provider_api_key.as_deref(),
        Some("gemini-test-key")
    );
    assert_eq!(
        args.base_url.as_deref(),
        Some("https://generativelanguage.googleapis.com/v1beta")
    );
    assert!(args.skip_quota_check);
    let rendered = rendered_codex_args(&args);
    assert!(rendered.contains(&"model_provider=\"prodex-gemini\"".to_string()));
    assert!(rendered.contains(&"model=\"auto\"".to_string()));
    assert!(rendered.contains(&"model_providers.prodex-gemini.name=\"Azure\"".to_string()));
    assert!(rendered.contains(
        &"model_providers.prodex-gemini.base_url=\"https://generativelanguage.googleapis.com/v1beta\""
            .to_string()
    ));
    assert!(rendered.contains(&"model_providers.prodex-gemini.wire_api=\"responses\"".to_string()));
    assert!(rendered.contains(&"model_context_window=1048576".to_string()));
    assert!(rendered.contains(&"model_auto_compact_token_limit=900000".to_string()));
    assert!(rendered.contains(&"model_reasoning_summary=\"none\"".to_string()));
    assert!(rendered.contains(&"model_supports_reasoning_summaries=true".to_string()));
    assert!(rendered.contains(&"web_search=\"live\"".to_string()));
    assert!(rendered.contains(&"features.apps=false".to_string()));
    assert!(rendered.contains(&"features.image_generation=true".to_string()));
    assert!(!rendered.iter().any(|arg| arg.contains("gemini-test-key")));
}
#[test]
fn caveman_command_keeps_smart_context_autopilot_disabled() {
    let command = parse_cli_command_from(["prodex", "caveman", "exec", "hello"])
        .expect("caveman command should parse");
    let Commands::Caveman(args) = command else {
        panic!("expected caveman command");
    };
    assert!(!args.smart_context);
    assert!(!args.super_optimizer_overlay);
}
#[test]
fn optimizer_shortcuts_parse_as_top_level_commands_not_run_passthrough() {
    for (command_name, expected) in [
        ("rtk", "rtk"),
        ("sqz", "sqz"),
        ("tokensavior", "tokensavior"),
        ("token-savior", "tokensavior"),
        ("clawcompactor", "clawcompactor"),
        ("claw-compactor", "clawcompactor"),
        ("mem", "mem"),
        ("memory", "mem"),
    ] {
        assert!(!should_default_cli_invocation_to_run(&os_args(&[
            "prodex",
            command_name,
        ])));
        let command = parse_cli_command_from(["prodex", command_name, "exec", "hello"])
            .expect("optimizer shortcut should parse");
        let args = match command {
            Commands::Rtk(args)
            | Commands::Sqz(args)
            | Commands::TokenSavior(args)
            | Commands::ClawCompactor(args)
            | Commands::Mem(args) => args,
            other => panic!("expected optimizer shortcut command, got {other:?}"),
        };
        assert_eq!(args.codex_args, os_args(&["exec", "hello"]));
        assert_eq!(
            caveman_args_with_optimizer_prefix(args, expected).codex_args,
            os_args(&[expected, "exec", "hello"])
        );
    }
}
#[test]
fn codex_remote_control_defaults_to_managed_run_passthrough() {
    assert!(should_default_cli_invocation_to_run(&os_args(&[
        "prodex",
        "remote-control",
        "--help",
    ])));
    let command = parse_cli_command_from(["prodex", "remote-control", "--help"])
        .expect("remote-control should parse as run passthrough");
    let Commands::Run(args) = command else {
        panic!("expected run command");
    };
    assert_eq!(args.codex_args, os_args(&["remote-control", "--help"]));
}
#[test]
fn codex_archive_commands_default_to_managed_run_passthrough() {
    for passthrough in [
        ["archive", "019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9"],
        ["unarchive", "019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9"],
    ] {
        let mut raw_args = vec![OsString::from("prodex")];
        raw_args.extend(passthrough.iter().map(|value| OsString::from(*value)));
        assert!(should_default_cli_invocation_to_run(&raw_args));
        let command = parse_cli_command_from(raw_args).expect("codex archive command should parse");
        let Commands::Run(args) = command else {
            panic!("expected run command");
        };
        assert_eq!(
            args.codex_args,
            passthrough
                .iter()
                .map(|value| OsString::from(*value))
                .collect::<Vec<_>>()
        );
    }
}
#[test]
fn codex_delete_defaults_to_managed_run_passthrough() {
    assert!(should_default_cli_invocation_to_run(&os_args(&[
        "prodex",
        "delete",
        "019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9",
    ])));
    let command =
        parse_cli_command_from(["prodex", "delete", "019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9"])
            .expect("codex delete should parse as run passthrough");
    let Commands::Run(args) = command else {
        panic!("expected run command");
    };
    assert_eq!(
        args.codex_args,
        os_args(&["delete", "019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9"])
    );
}
#[test]
fn codex_exec_resume_output_schema_defaults_to_managed_run_passthrough() {
    assert!(should_default_cli_invocation_to_run(&os_args(&[
        "prodex",
        "exec",
        "resume",
        "019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9",
        "--output-schema",
        "schema.json",
        "return json",
    ])));
    let command = parse_cli_command_from([
        "prodex",
        "exec",
        "resume",
        "019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9",
        "--output-schema",
        "schema.json",
        "return json",
    ])
    .expect("exec resume should parse as run passthrough");
    let Commands::Run(args) = command else {
        panic!("expected run command");
    };
    assert_eq!(
        args.codex_args,
        os_args(&[
            "exec",
            "resume",
            "019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9",
            "--output-schema",
            "schema.json",
            "return json",
        ])
    );
}
#[test]
fn codex_command_server_subcommands_default_to_run_passthrough() {
    for subcommand in ["mcp-server", "app-server", "exec-server"] {
        assert!(should_default_cli_invocation_to_run(&os_args(&[
            "prodex", subcommand, "--help",
        ])));
        let command = parse_cli_command_from(["prodex", subcommand, "--help"])
            .expect("command-server subcommand should parse as run passthrough");
        let Commands::Run(args) = command else {
            panic!("expected run command");
        };
        assert_eq!(args.codex_args, os_args(&[subcommand, "--help"]));
    }
}
#[test]
fn codex_mcp_server_meta_args_are_exact_run_passthrough() {
    let passthrough = [
        "mcp-server",
        "--stdio",
        "--method",
        "tools/call",
        "--params",
        r#"{"_meta":{"trace_id":"trace-smoke"},"meta":{"compat":"codex-rust-v0.132.0"},"arguments":{"image":{"detail":"low"}}}"#,
    ];
    let mut argv = vec!["prodex"];
    argv.extend(passthrough);
    let command = parse_cli_command_from(argv).expect("mcp-server args should parse as run");
    let Commands::Run(args) = command else {
        panic!("expected run command");
    };
    assert_eq!(args.codex_args, os_args(&passthrough));
}
#[test]
fn codex_command_server_detection_is_first_arg_only() {
    assert!(is_codex_command_server_subcommand(&os_args(&[
        "mcp-server",
        "--stdio",
    ])));
    assert!(is_codex_command_server_subcommand(&os_args(&[
        "app-server",
        "--stdio",
    ])));
    assert!(is_codex_command_server_subcommand(&os_args(&[
        "exec-server",
        "--stdio",
    ])));
    assert!(!is_codex_command_server_subcommand(&os_args(&[
        "remote-control",
    ])));
    assert!(!is_codex_command_server_subcommand(&os_args(&[
        "exec",
        "mcp-server",
    ])));
    assert!(!is_codex_command_server_subcommand(&[]));
}
#[test]
fn session_list_parses_line_modes_and_filters() {
    let command = parse_cli_command_from([
        "prodex",
        "session",
        "list",
        "--id-only",
        "--profile",
        "main",
        "--query",
        "triage",
        "--limit",
        "5",
        "--include-subagents",
    ])
    .expect("session list should parse");
    let Commands::Session(SessionCommands::List(args)) = command else {
        panic!("expected session list command");
    };
    assert!(args.id_only);
    assert!(!args.resume_command);
    assert_eq!(args.profile.as_deref(), Some("main"));
    assert_eq!(args.query.as_deref(), Some("triage"));
    assert_eq!(args.limit, Some(5));
    assert!(args.include_subagents);
}
#[test]
fn session_current_parses_resume_command_filters_and_cwd() {
    let command = parse_cli_command_from([
        "prodex",
        "session",
        "current",
        "--resume-command",
        "--profile",
        "main",
        "--query",
        "triage",
        "--cwd",
        "/tmp/work",
    ])
    .expect("session current should parse");
    let Commands::Session(SessionCommands::Current(args)) = command else {
        panic!("expected session current command");
    };
    assert!(!args.id_only);
    assert!(args.resume_command);
    assert_eq!(args.profile.as_deref(), Some("main"));
    assert_eq!(args.query.as_deref(), Some("triage"));
    assert_eq!(args.cwd.as_deref(), Some(std::path::Path::new("/tmp/work")));
    assert!(!args.include_subagents);
}
#[test]
fn session_line_modes_conflict_with_json_and_each_other() {
    assert!(parse_cli_command_from(["prodex", "session", "list", "--json", "--id-only"]).is_err());
    assert!(
        parse_cli_command_from([
            "prodex",
            "session",
            "current",
            "--id-only",
            "--resume-command",
        ])
        .is_err()
    );
}
#[test]
fn session_resume_parses_partial_id() {
    let command = parse_cli_command_from(["prodex", "session", "resume", "1234abcd"])
        .expect("session resume should parse");
    let Commands::Session(SessionCommands::Resume(args)) = command else {
        panic!("expected session resume command");
    };
    assert_eq!(args.id, "1234abcd");
}
