use super::*;

#[test]
fn context_audit_command_accepts_json_and_root() {
    let command = parse_cli_command_from([
        "prodex",
        "context",
        "audit",
        "--root",
        "/tmp/codex-context",
        "--limit",
        "7",
        "--json",
    ])
    .expect("context audit command");
    let Commands::Context(ContextCommands::Audit(args)) = command else {
        panic!("expected context audit command");
    };
    assert_eq!(args.root.as_deref(), Some(Path::new("/tmp/codex-context")));
    assert_eq!(args.limit, 7);
    assert!(args.json);
}

#[test]
fn context_export_command_accepts_id_and_optional_path() {
    let command = parse_cli_command_from([
        "prodex",
        "context",
        "export",
        "019c9e3d",
        "./context_session.md",
    ])
    .expect("context export command");
    let Commands::Context(ContextCommands::Export(args)) = command else {
        panic!("expected context export command");
    };
    assert_eq!(args.id, "019c9e3d");
    assert_eq!(args.path.as_deref(), Some(Path::new("./context_session.md")));
}

#[test]
fn context_compact_output_command_accepts_kind_limits_and_json() {
    let command = parse_cli_command_from([
        "prodex",
        "context",
        "compact-output",
        "/tmp/prodex-output.txt",
        "--kind",
        "git-diff",
        "--max-lines",
        "24",
        "--head-lines",
        "10",
        "--tail-lines",
        "4",
        "--max-line-chars",
        "120",
        "--max-search-matches-per-file",
        "2",
        "--max-path-entries",
        "12",
        "--json",
    ])
    .expect("context compact-output command");
    let Commands::Context(ContextCommands::CompactOutput(args)) = command else {
        panic!("expected context compact-output command");
    };
    assert_eq!(args.path.as_deref(), Some(Path::new("/tmp/prodex-output.txt")));
    assert!(matches!(args.kind, ContextCompactOutputKind::GitDiff));
    assert_eq!(args.max_lines, 24);
    assert_eq!(args.head_lines, 10);
    assert_eq!(args.tail_lines, 4);
    assert_eq!(args.max_line_chars, 120);
    assert_eq!(args.max_search_matches_per_file, 2);
    assert_eq!(args.max_path_entries, 12);
    assert!(args.json);
}

#[test]
fn context_replay_report_command_accepts_path_and_json() {
    let command = parse_cli_command_from([
        "prodex",
        "context",
        "replay-report",
        "/tmp/smart-context-replay.json",
        "--json",
        "--strict",
    ])
    .expect("context replay-report command");
    let Commands::Context(ContextCommands::ReplayReport(args)) = command else {
        panic!("expected context replay-report command");
    };
    assert_eq!(
        args.path.as_path(),
        Path::new("/tmp/smart-context-replay.json")
    );
    assert!(args.json);
    assert!(args.strict);
}

#[test]
fn context_replay_report_fixture_renders_markdown() {
    let corpus_text = include_str!(
        "../../../../prodex-runtime-proxy/tests/fixtures/smart_context_replay_corpus.json"
    );

    let replay = run_runtime_smart_context_replay_json(corpus_text)
        .expect("replay corpus should execute");
    let report = render_runtime_smart_context_replay_markdown(&replay);

    assert!(report.contains("# Smart Context Deterministic Replay"));
    assert!(report.contains("- passed: true"));
    assert!(report.contains("within-request-duplicate-build-output"));
}

#[test]
fn context_replay_report_supports_every_registered_provider() {
    for provider in [
        "openai",
        "anthropic",
        "copilot",
        "deepseek",
        "gemini",
        "kiro",
        "local",
    ] {
        let corpus = serde_json::json!({
            "schema_version": 2,
            "scenarios": [{
                "id": format!("{provider}-exact"),
                "transport": "http",
                "route": "responses",
                "provider": provider,
                "model": "gpt-5.1-codex",
                "context_window_tokens": 16384,
                "mode": "exact",
                "turns": [{
                    "request": {"model": "gpt-5.1-codex", "input": "hello"},
                    "expected_outcome": "pass_through"
                }]
            }]
        });
        let report = run_runtime_smart_context_replay_json(&corpus.to_string())
            .unwrap_or_else(|error| panic!("{provider} replay failed: {error:#}"));
        assert!(report.passed, "{provider} replay should pass");
        assert_eq!(report.scenarios[0].provider, provider);
    }
}

#[test]
fn context_replay_report_strict_rejects_failed_corpus() {
    let temp_dir = TestDir::new();
    let corpus_path = temp_dir.path.join("failed-replay.json");
    fs::write(
        &corpus_path,
        r#"{
  "schema_version": 2,
  "scenarios": [{
    "id": "failed",
    "transport": "http",
    "route": "responses",
    "provider": "openai",
    "model": "gpt-5.1-codex",
    "context_window_tokens": 16384,
    "mode": "exact",
    "turns": [{
      "request": {"model": "gpt-5.1-codex", "input": []},
      "expected_outcome": "rewrite"
    }]
  }]
}"#,
    )
    .expect("failed corpus should be written");

    let err = handle_context_replay_report(ContextReplayReportArgs {
        path: corpus_path.clone(),
        json: false,
        strict: true,
    })
    .expect_err("strict failed corpus should fail");

    assert!(err.to_string().contains("replay acceptance failed"));
}

#[test]
fn context_audit_reports_shared_context_roots() {
    let temp_dir = TestDir::new();
    let root = temp_dir.path.join("codex");
    fs::create_dir_all(root.join("skills/review")).expect("skills dir should be created");
    fs::write(
        root.join("AGENTS.md"),
        "This repository has a very detailed instruction paragraph for every worker.\n",
    )
    .expect("agents file should be written");
    fs::write(
        root.join("skills/review/SKILL.md"),
        "Use this skill in order to review code carefully and report risks.\n",
    )
    .expect("skill file should be written");
    fs::write(
        root.join("skills/review/SKILL.original.md"),
        "backup should be skipped\n",
    )
    .expect("backup should be written");

    let report = collect_context_audit_report(&root, 20).expect("audit should succeed");
    let paths = report
        .files
        .iter()
        .map(|entry| entry.relative_path.as_str())
        .collect::<Vec<_>>();

    assert!(paths.contains(&"AGENTS.md"));
    assert!(paths.contains(&"skills/review/SKILL.md"));
    assert!(!paths.contains(&"skills/review/SKILL.original.md"));
    assert!(report.total_estimated_tokens > 0);
}

#[test]
fn context_compress_creates_backup_and_preserves_protected_lines() {
    let temp_dir = TestDir::new();
    let path = temp_dir.path.join("AGENTS.md");
    fs::write(
        &path,
        concat!(
            "# Keep Title\n\n",
            "This is actually a very verbose paragraph in order to make sure to reduce tokens. ",
            "It is important to please note that this sentence is really redundant.\n\n",
            "Run `cargo test` before shipping.\n\n",
            "```bash\n",
            "cargo test -q\n",
            "```\n"
        ),
    )
    .expect("context file should be written");

    let report = compress_context_path(&path, false).expect("compress should succeed");
    assert_eq!(report.entries.len(), 1);
    assert_eq!(report.entries[0].status, "compressed");

    let backup = temp_dir.path.join("AGENTS.original.md");
    assert!(backup.exists());
    let compressed = fs::read_to_string(&path).expect("compressed file should be readable");
    assert!(compressed.contains("# Keep Title"));
    assert!(compressed.contains("Run `cargo test` before shipping."));
    assert!(compressed.contains("```bash\ncargo test -q\n```"));
    assert!(!compressed.contains("actually"));
    assert!(compressed.len() < fs::read_to_string(&backup).unwrap().len());

    let rerun = compress_context_path(&path, false).expect("second compress should succeed");
    assert_eq!(rerun.entries[0].status, "skipped_backup_exists");
}

#[test]
fn context_compress_skips_non_prose_and_backups() {
    let temp_dir = TestDir::new();
    let json_path = temp_dir.path.join("config.json");
    let backup_path = temp_dir.path.join("notes.original.md");
    fs::write(&json_path, "{}").expect("json should be written");
    fs::write(&backup_path, "backup").expect("backup should be written");

    let report = compress_context_path(&temp_dir.path, false).expect("compress should succeed");
    let statuses = report
        .entries
        .iter()
        .map(|entry| (entry.path.file_name().unwrap().to_string_lossy(), entry.status.as_str()))
        .collect::<Vec<_>>();

    assert!(statuses.contains(&("config.json".into(), "skipped_not_prose")));
    assert!(statuses.contains(&("notes.original.md".into(), "skipped_not_prose")));
}
