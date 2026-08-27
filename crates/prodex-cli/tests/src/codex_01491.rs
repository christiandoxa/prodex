use super::*;

const THREAD_SOURCE: &str = "automated_review";
const IMAGE_BUDGET: &str = "features.compaction_image_budget=true";

fn passthrough_args(args: &[&str]) -> Vec<OsString> {
    match parse_cli_command_from(args.iter().copied()).expect("managed launch should parse") {
        Commands::Run(args) => args.codex_args,
        Commands::Caveman(args) => args.codex_args,
        Commands::Super(args) => args.codex_args,
        command => panic!("unexpected command: {command:?}"),
    }
}

#[test]
fn codex_01491_thread_source_is_exact_passthrough_for_managed_launches() {
    let expected = os_args(&[
        "exec",
        "--thread-source",
        THREAD_SOURCE,
        "review this repository",
    ]);
    for invocation in [
        vec![
            "prodex",
            "exec",
            "--thread-source",
            THREAD_SOURCE,
            "review this repository",
        ],
        vec![
            "prodex",
            "run",
            "exec",
            "--thread-source",
            THREAD_SOURCE,
            "review this repository",
        ],
        vec![
            "prodex",
            "caveman",
            "exec",
            "--thread-source",
            THREAD_SOURCE,
            "review this repository",
        ],
        vec![
            "prodex",
            "s",
            "--no-presidio",
            "--no-sub-agent",
            "exec",
            "--thread-source",
            THREAD_SOURCE,
            "review this repository",
        ],
    ] {
        assert_eq!(passthrough_args(&invocation), expected, "{invocation:?}");
    }
}

#[test]
fn codex_01491_exec_fork_preserves_every_global_thread_source_position() {
    for passthrough in [
        vec![
            "exec",
            "--thread-source",
            THREAD_SOURCE,
            "fork",
            "thread-123",
            "-",
        ],
        vec![
            "exec",
            "fork",
            "--thread-source",
            THREAD_SOURCE,
            "thread-123",
            "-",
        ],
        vec![
            "exec",
            "fork",
            "thread-123",
            "--thread-source",
            THREAD_SOURCE,
            "-",
        ],
    ] {
        let mut invocation = vec!["prodex"];
        invocation.extend(passthrough.iter().copied());
        assert_eq!(passthrough_args(&invocation), os_args(&passthrough));
    }
}

#[test]
fn codex_01491_resume_and_stdin_receive_no_generated_thread_source() {
    assert_eq!(
        passthrough_args(&["prodex", "exec", "resume", "thread-123", "continue",]),
        os_args(&["exec", "resume", "thread-123", "continue"])
    );
    assert_eq!(
        passthrough_args(&["prodex", "exec", "--thread-source", THREAD_SOURCE, "-",]),
        os_args(&["exec", "--thread-source", THREAD_SOURCE, "-"])
    );
}

#[test]
fn codex_01491_image_budget_override_reaches_each_codex_launch_once() {
    for invocation in [
        vec!["prodex", "-c", IMAGE_BUDGET, "exec", "review"],
        vec!["prodex", "run", "-c", IMAGE_BUDGET, "exec", "review"],
        vec!["prodex", "caveman", "-c", IMAGE_BUDGET, "exec", "review"],
    ] {
        assert_eq!(
            passthrough_args(&invocation),
            os_args(&["-c", IMAGE_BUDGET, "exec", "review"]),
            "{invocation:?}"
        );
    }

    let command = parse_cli_command_from([
        "prodex",
        "s",
        "--no-presidio",
        "--no-sub-agent",
        "-c",
        IMAGE_BUDGET,
        "exec",
        "review",
    ])
    .expect("Super launch should parse");
    let Commands::Super(args) = command else {
        panic!("expected Super command");
    };
    assert_eq!(
        args.into_runtime_tool_args().codex_args,
        os_args(&[
            "-c",
            "features.apps=false",
            "-c",
            IMAGE_BUDGET,
            "exec",
            "review",
        ])
    );
}

#[test]
fn codex_01491_provider_defaults_precede_explicit_image_budget_overrides() {
    let command = parse_cli_command_from([
        "prodex",
        "s",
        "--provider",
        "gemini",
        "--api-key",
        "test-key",
        "-c",
        "features.compaction_image_budget=false",
        "-c",
        IMAGE_BUDGET,
        "exec",
        "review",
    ])
    .expect("provider-backed Super launch should parse");
    let Commands::Super(args) = command else {
        panic!("expected Super command");
    };
    let rendered = args
        .into_runtime_tool_args()
        .codex_args
        .into_iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>();

    assert_eq!(
        &rendered[rendered.len() - 6..],
        [
            "-c",
            "features.compaction_image_budget=false",
            "-c",
            IMAGE_BUDGET,
            "exec",
            "review",
        ]
    );
    assert_eq!(
        rendered
            .iter()
            .filter(|arg| arg.as_str() == IMAGE_BUDGET)
            .count(),
        1
    );

    let command = parse_cli_command_from([
        "prodex",
        "s",
        "--no-presidio",
        "--no-sub-agent",
        "exec",
        "review",
    ])
    .expect("unspecified Super launch should parse");
    let Commands::Super(args) = command else {
        panic!("expected Super command");
    };
    assert!(args.into_runtime_tool_args().codex_args.iter().all(|arg| {
        !arg.to_string_lossy()
            .starts_with("features.compaction_image_budget=")
    }));
}

#[test]
fn codex_01501_unspecified_image_budget_leaves_the_upstream_default_owned_by_codex() {
    for invocation in [
        ["prodex", "exec", "review"].as_slice(),
        ["prodex", "run", "exec", "review"].as_slice(),
        ["prodex", "caveman", "exec", "review"].as_slice(),
        [
            "prodex",
            "s",
            "--no-presidio",
            "--no-sub-agent",
            "exec",
            "review",
        ]
        .as_slice(),
    ] {
        assert!(
            passthrough_args(invocation).iter().all(|arg| !arg
                .to_string_lossy()
                .starts_with("features.compaction_image_budget=")),
            "Prodex must not inject an image-budget default: {invocation:?}"
        );
    }
}

#[test]
fn codex_01501_explicit_image_budget_values_are_preserved() {
    for value in ["true", "false"] {
        let setting = format!("features.compaction_image_budget={value}");
        let command = parse_cli_command_from([
            "prodex",
            "s",
            "--no-presidio",
            "--no-sub-agent",
            "-c",
            setting.as_str(),
            "exec",
            "review",
        ])
        .expect("explicit Codex feature setting should parse");
        let Commands::Super(args) = command else {
            panic!("expected Super command");
        };
        let rendered = args
            .into_runtime_tool_args()
            .codex_args
            .into_iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert_eq!(
            rendered
                .iter()
                .filter(|arg| arg.as_str() == setting)
                .count(),
            1,
            "explicit image-budget setting must survive unchanged"
        );
    }
}
