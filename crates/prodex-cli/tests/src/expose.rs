use super::*;

#[test]
fn s_expose_rewrites_to_expose_command() {
    assert!(!should_default_cli_invocation_to_run(&os_args(&[
        "prodex", "expose",
    ])));

    let command = parse_cli_command_from([
        "prodex", "s", "expose", "--tunnel", "--cols", "120", "--rows", "40",
    ])
    .expect("super expose alias should parse");
    let Commands::Expose(args) = command else {
        panic!("expected expose command");
    };
    assert!(args.tunnel);
    assert!(!args.no_tunnel);
    assert_eq!(args.invocation, ExposeInvocation::SuperAlias);
    assert!(args.super_args.is_some());
    assert_eq!(args.cols, 120);
    assert_eq!(args.rows, 40);
}

#[test]
fn super_expose_captures_super_overrides_without_a_public_compatibility_flag() {
    let Commands::Expose(args) = parse_cli_command_from([
        "prodex",
        "s",
        "expose",
        "--name",
        "api",
        "--model",
        "gpt-5.6-luna",
        "-c",
        "model_reasoning_effort=max",
        "--no-sub-agent",
    ])
    .expect("Super expose overrides should parse") else {
        panic!("expected expose command");
    };
    assert_eq!(args.name.as_deref(), Some("api"));
    assert_eq!(args.invocation, ExposeInvocation::SuperAlias);
    let super_args = args.super_args.expect("Super args should be captured");
    assert_eq!(super_args.local_model.as_deref(), Some("gpt-5.6-luna"));
    assert!(super_args.no_sub_agent);
    assert!(
        super_args
            .codex_args
            .windows(2)
            .any(|pair| pair == ["-c", "model_reasoning_effort=max"])
    );
}

#[test]
fn super_expose_normalization_preserves_options_before_and_after_alias() {
    let Commands::Expose(args) = parse_cli_command_from([
        "prodex",
        "s",
        "--no-presidio",
        "--model",
        "model-before-expose",
        "expose",
        "--name",
        "expose",
        "--no-tunnel",
    ])
    .expect("positioned Super expose options should parse") else {
        panic!("expected expose command");
    };
    assert_eq!(args.name.as_deref(), Some("expose"));
    assert!(args.no_tunnel);
    let super_args = args.super_args.expect("Super args should be captured");
    assert!(super_args.no_presidio);
    assert_eq!(
        super_args.local_model.as_deref(),
        Some("model-before-expose")
    );
}

#[test]
fn top_level_expose_remains_standalone() {
    let Commands::Expose(args) =
        parse_cli_command_from(["prodex", "expose"]).expect("standalone expose should parse")
    else {
        panic!("expected expose command");
    };
    assert_eq!(args.invocation, ExposeInvocation::Standalone);
    assert!(args.super_args.is_none());
}

#[test]
fn super_exec_task_named_expose_is_not_normalized_as_the_expose_command() {
    let Commands::Super(args) =
        parse_cli_command_from(["prodex", "s", "exec", "expose"]).expect("Super exec should parse")
    else {
        panic!("expected Super command");
    };
    assert_eq!(args.codex_args, ["exec", "expose"]);
}

#[test]
fn expose_tunnel_is_opt_in_and_legacy_no_tunnel_is_unambiguous() {
    let Commands::Expose(defaults) =
        parse_cli_command_from(["prodex", "expose"]).expect("expose defaults should parse")
    else {
        panic!("expected expose command");
    };
    assert!(!defaults.tunnel);
    assert!(!defaults.no_tunnel);

    let Commands::Expose(legacy) = parse_cli_command_from(["prodex", "expose", "--no-tunnel"])
        .expect("legacy no-tunnel alias should parse")
    else {
        panic!("expected expose command");
    };
    assert!(!legacy.tunnel);
    assert!(legacy.no_tunnel);

    assert!(parse_cli_command_from(["prodex", "expose", "--tunnel", "--no-tunnel"]).is_err());
    assert!(parse_cli_command_from(["prodex", "expose", "--max-clients", "0"]).is_err());
    assert!(parse_cli_command_from(["prodex", "expose", "--max-clients", "33"]).is_err());
}
