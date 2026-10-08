use super::*;

#[test]
fn codex_0161_program_is_exact_passthrough_for_each_managed_launch() {
    for prefix in [
        vec!["prodex"],
        vec!["prodex", "run"],
        vec!["prodex", "s", "--no-presidio", "--no-sub-agent"],
    ] {
        for tail in [
            vec![
                "exec",
                "--cyber-access-program",
                "standard",
                "resume",
                "thread-test",
                "continue",
            ],
            vec![
                "exec",
                "resume",
                "--cyber-access-program",
                "daybreak_blue",
                "thread-test",
                "-",
            ],
            vec![
                "exec",
                "fork",
                "thread-test",
                "--cyber-access-program=daybreak_red",
                "continue",
            ],
        ] {
            let mut invocation = prefix.clone();
            invocation.extend(&tail);
            let actual = match parse_cli_command_from(invocation).unwrap() {
                Commands::Run(args) => args.codex_args,
                Commands::Super(args) => args.codex_args,
                other => panic!("unexpected command: {other:?}"),
            };
            assert_eq!(actual, os_args(&tail));
        }
    }
}

#[test]
fn codex_0161_super_keeps_daybreak_opt_in_and_explicit_override_precedence() {
    for tail in [
        vec!["exec", "review this code"],
        vec!["-c", "daybreak=true", "exec", "review this code"],
        vec![
            "--enable",
            "cli_daybreak",
            "-c",
            "daybreak=false",
            "-c",
            "daybreak=true",
            "exec",
            "review this code",
        ],
        vec![
            "--disable",
            "cli_daybreak",
            "exec",
            "--cyber-access-program",
            "standard",
            "review this code",
        ],
        vec![
            "-c",
            "features.cli_daybreak=false",
            "exec",
            "--",
            "literal --cyber-access-program text",
        ],
    ] {
        let mut invocation = vec!["prodex", "s", "--no-presidio", "--no-sub-agent"];
        invocation.extend(&tail);
        let mut expected = os_args(&["-c", "features.apps=false"]);
        expected.extend(os_args(&tail));
        assert_eq!(
            parse_super_as_runtime_tools(&invocation).codex_args,
            expected
        );
    }
}

#[test]
fn codex_0161_super_preserves_explicit_sol_model_and_reasoning_without_defaults() {
    let args = parse_super_as_runtime_tools(&[
        "prodex",
        "s",
        "--no-presidio",
        "--no-sub-agent",
        "--model",
        "gpt-6.1-sol",
        "-c",
        "model_reasoning_effort=\"ultra\"",
        "exec",
        "review this code",
    ]);
    let rendered = rendered_codex_args(&args);
    assert_eq!(
        rendered
            .iter()
            .filter(|value| *value == "model=\"gpt-6.1-sol\"")
            .count(),
        1
    );
    assert!(rendered.ends_with(&[
        "-c".to_string(),
        "model_reasoning_effort=\"ultra\"".to_string(),
        "exec".to_string(),
        "review this code".to_string(),
    ]));
    assert!(!rendered.iter().any(|value| value.contains("daybreak")));
}
